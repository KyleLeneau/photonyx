//! An ordered set of frames on disk, treated as one image sequence
//! (ADR 007, D6 / P1-T2).
//!
//! Stages read the same region from every frame in turn, so what they need is
//! not a loaded sequence but a cheap, validated description of one: the paths
//! in a fixed order, their shared geometry, and a header per frame. Pixels are
//! read on demand, a block at a time, and never all held at once.

use std::path::{Path, PathBuf};

use px_fits::header::Header;
use px_fits::reader::FitsReader;
use px_fits::source::FileSource;
use px_fits::{BitPix, Region};

use crate::error::ImageProcError;
use crate::frame::{Frame, ValueScale};
use crate::runtime::{Block, ImageDims};

/// One frame's metadata, read once when the set is opened.
#[derive(Debug, Clone)]
pub struct FrameInfo {
    pub path: PathBuf,
    pub dims: ImageDims,
    pub bitpix: BitPix,
    pub header: Header,
    /// Decided once, from the header and (for float frames) a single sampled
    /// row, so every block of this frame is scaled identically.
    pub scale: ValueScale,
}

/// An ordered, geometrically uniform set of frames.
///
/// Uniformity is checked up front rather than discovered halfway through a
/// stack: a set that mixes sizes, channel counts, or bit depths cannot be
/// reduced pixel-wise, and finding that out after an hour of reading is the
/// bad outcome.
#[derive(Debug, Clone)]
pub struct FrameSet {
    frames: Vec<FrameInfo>,
    dims: ImageDims,
    bitpix: BitPix,
}

impl FrameSet {
    /// Opens every path, reads its header, and checks the set is uniform.
    ///
    /// Order is preserved exactly as given — it determines frame indices, and
    /// those indices appear in registration data and rejection maps.
    pub fn open(paths: &[PathBuf]) -> Result<Self, ImageProcError> {
        if paths.is_empty() {
            return Err(ImageProcError::EmptySequence);
        }

        let mut frames = Vec::with_capacity(paths.len());
        for path in paths {
            frames.push(read_info(path)?);
        }

        let first = &frames[0];
        let (dims, bitpix) = (first.dims, first.bitpix);

        for info in &frames[1..] {
            if info.dims != dims {
                return Err(ImageProcError::NonUniformSequence {
                    path: info.path.clone(),
                    detail: format!(
                        "is {}x{}x{} but {} is {}x{}x{}",
                        info.dims.width,
                        info.dims.height,
                        info.dims.channels,
                        first.path.display(),
                        dims.width,
                        dims.height,
                        dims.channels
                    ),
                });
            }
        }

        Ok(Self {
            frames,
            dims,
            bitpix,
        })
    }

    pub fn len(&self) -> usize {
        self.frames.len()
    }

    pub fn is_empty(&self) -> bool {
        self.frames.is_empty()
    }

    /// The geometry every frame in the set shares.
    pub fn dims(&self) -> ImageDims {
        self.dims
    }

    /// The `BITPIX` of the first frame. Frames may differ here — mixed depths
    /// still stack correctly once everything is unit-range `f32` — so this is
    /// informational, used for output defaults and reporting.
    pub fn bitpix(&self) -> BitPix {
        self.bitpix
    }

    pub fn frames(&self) -> &[FrameInfo] {
        &self.frames
    }

    pub fn get(&self, index: usize) -> Option<&FrameInfo> {
        self.frames.get(index)
    }

    pub fn paths(&self) -> impl Iterator<Item = &Path> {
        self.frames.iter().map(|f| f.path.as_path())
    }

    /// Bytes one whole frame occupies once decoded, for budgeting the stages
    /// that work a frame at a time.
    pub fn frame_bytes(&self) -> u64 {
        self.dims.pixel_count() as u64 * std::mem::size_of::<f32>() as u64
    }

    /// Loads one frame in full.
    pub fn load(&self, index: usize) -> Result<Frame, ImageProcError> {
        let info = self.frames.get(index).ok_or_else(|| {
            ImageProcError::Processing(format!(
                "frame index {index} out of range for a {}-frame set",
                self.frames.len()
            ))
        })?;
        let reader = FitsReader::open(&info.path)?;
        let hdu = reader.primary_image()?;
        Frame::from_hdu(&hdu)
    }

    /// Reads one [`Block`] from one frame into `out`, which must hold exactly
    /// `block.pixel_count(dims)` samples.
    ///
    /// This is the read the block-scheduled reductions make, once per frame
    /// per block. It touches only the block's bytes.
    pub fn read_block_into(
        &self,
        index: usize,
        block: &Block,
        out: &mut [f32],
    ) -> Result<(), ImageProcError> {
        let expected = block.pixel_count(&self.dims);
        if out.len() != expected {
            return Err(ImageProcError::Processing(format!(
                "block buffer holds {} samples, need {expected}",
                out.len()
            )));
        }

        let info = self.frames.get(index).ok_or_else(|| {
            ImageProcError::Processing(format!(
                "frame index {index} out of range for a {}-frame set",
                self.frames.len()
            ))
        })?;

        let reader = FitsReader::open(&info.path)?;
        let hdu = reader.primary_image()?;

        let region = if self.dims.channels > 1 {
            Region::new(
                vec![0, block.start_row, block.channel],
                vec![self.dims.width, block.height(), 1],
            )
        } else {
            Region::rect(0, block.start_row, self.dims.width, block.height())
        };
        hdu.read_region_into(&region, out)?;
        crate::frame::apply_scale(out, info.scale);
        Ok(())
    }
}

fn read_info(path: &Path) -> Result<FrameInfo, ImageProcError> {
    let reader: FitsReader<FileSource> = FitsReader::open(path)?;
    let hdu = reader.primary_image()?;

    let dims = match hdu.shape() {
        [w, h] => ImageDims::new(*w, *h, 1),
        [w, h, c] if *c == 1 || *c == 3 => ImageDims::new(*w, *h, *c),
        other => {
            return Err(ImageProcError::NonUniformSequence {
                path: path.to_path_buf(),
                detail: format!("has unsupported image shape {other:?}"),
            });
        }
    };

    Ok(FrameInfo {
        path: path.to_path_buf(),
        dims,
        bitpix: hdu.bitpix(),
        header: hdu.header().clone(),
        scale: crate::frame::probe_value_scale(&hdu)?,
    })
}
