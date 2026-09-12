//! The in-memory image every stage of the native engine operates on
//! (ADR 007, D1 / P1-T1).
//!
//! Pixels are `f32` in the unit range, planar and channel-major: all of
//! channel 0's rows, then channel 1's, then channel 2's. That layout matches
//! the FITS axis order `px-fits` reads in, so a channel is a contiguous
//! slice and a block of rows within a channel is a contiguous sub-slice —
//! which is what the block scheduler in [`crate::runtime`] hands out.

use px_fits::header::Header;
use px_fits::image::ImageHdu;
use px_fits::source::ByteSource;
use px_fits::{BitPix, FitsWriter, HeaderBuilder, Region};

use crate::error::ImageProcError;
use crate::runtime::ImageDims;

/// Full-well value for integer sensor data. Astro cameras are 12–16 bit and
/// their FITS files are 16-bit integer, so this is the divisor that maps raw
/// counts onto the unit range.
pub const U16_MAX_F: f32 = 65535.0;

/// Above this, float input is assumed to be raw counts rather than already
/// normalized, and is divided down.
///
/// Frames that have been through a unit-range pipeline land in [0, 1] with the
/// occasional overshoot from interpolation or normalization; nothing in that
/// world reaches 10. Float frames straight off a camera or out of a converter
/// carry ADU in the thousands. There is no header keyword that distinguishes
/// them, so the magnitude is the only available signal.
const FLOAT_COUNTS_THRESHOLD: f32 = 10.0;

/// How raw physical values map onto the unit range.
///
/// `px-fits` has already applied `BSCALE`/`BZERO`, so this is only the final
/// photometric normalization.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ValueScale {
    /// Added before dividing. Non-zero only for genuinely signed integer data.
    pub offset: f32,
    /// Divisor mapping counts onto [0, 1]. 1.0 leaves values alone.
    pub divisor: f32,
}

impl ValueScale {
    /// The identity: values are already in the unit range.
    pub const UNIT: Self = Self {
        offset: 0.0,
        divisor: 1.0,
    };

    #[inline]
    fn apply(&self, v: f32) -> f32 {
        (v + self.offset) / self.divisor
    }

    fn is_identity(&self) -> bool {
        self.offset == 0.0 && self.divisor == 1.0
    }

    /// Picks the scale for an image, given its `BITPIX`, whether the header
    /// carried a `BZERO`, and — for float data only — the largest value
    /// present.
    fn choose(bitpix: BitPix, has_bzero: bool, float_max: Option<f32>) -> Self {
        match bitpix {
            BitPix::U8 => Self {
                offset: 0.0,
                divisor: 255.0,
            },
            BitPix::I16 => Self {
                // A signed frame with no BZERO spans -32768..32767; shifting it
                // up puts it on the same footing as the unsigned convention,
                // which BSCALE/BZERO has already handled when present.
                offset: if has_bzero { 0.0 } else { 32768.0 },
                divisor: U16_MAX_F,
            },
            BitPix::I32 | BitPix::I64 => Self {
                offset: 0.0,
                divisor: U16_MAX_F,
            },
            BitPix::F32 | BitPix::F64 => match float_max {
                Some(max) if max > FLOAT_COUNTS_THRESHOLD => Self {
                    offset: 0.0,
                    divisor: U16_MAX_F,
                },
                _ => Self::UNIT,
            },
        }
    }
}

/// A decoded image: unit-range `f32` samples plus the header they came from.
#[derive(Debug, Clone)]
pub struct Frame {
    dims: ImageDims,
    data: Vec<f32>,
    header: Header,
    /// The scale applied on load, so a round-trip back to the source
    /// representation is possible.
    scale: ValueScale,
}

impl Frame {
    /// Wraps an existing unit-range buffer. `data.len()` must be
    /// `dims.pixel_count()`.
    pub fn new(dims: ImageDims, data: Vec<f32>, header: Header) -> Result<Self, ImageProcError> {
        if data.len() != dims.pixel_count() {
            return Err(ImageProcError::Processing(format!(
                "buffer of {} samples does not match {}x{}x{}",
                data.len(),
                dims.width,
                dims.height,
                dims.channels
            )));
        }
        Ok(Self {
            dims,
            data,
            header,
            scale: ValueScale::UNIT,
        })
    }

    /// An all-zero frame of the given shape, carrying `header`.
    pub fn zeros(dims: ImageDims, header: Header) -> Self {
        Self {
            data: vec![0.0; dims.pixel_count()],
            dims,
            header,
            scale: ValueScale::UNIT,
        }
    }

    /// Decodes a whole image HDU.
    pub fn from_hdu<S: ByteSource + ?Sized>(hdu: &ImageHdu<'_, S>) -> Result<Self, ImageProcError> {
        let dims = dims_of(hdu)?;
        let scale = probe_value_scale(hdu)?;
        let mut data: Vec<f32> = hdu.read_full()?;
        apply_scale(&mut data, scale);
        Ok(Self {
            dims,
            data,
            header: hdu.header().clone(),
            scale,
        })
    }

    /// Decodes a rectangular window of an image HDU, in the source image's
    /// pixel coordinates. The result is a frame of the window's shape.
    ///
    /// The window applies to every channel: a 3-channel source yields a
    /// 3-channel frame.
    pub fn region_from_hdu<S: ByteSource + ?Sized>(
        hdu: &ImageHdu<'_, S>,
        x: usize,
        y: usize,
        width: usize,
        height: usize,
    ) -> Result<Self, ImageProcError> {
        let source_dims = dims_of(hdu)?;
        let region = if source_dims.channels > 1 {
            Region::new(vec![x, y, 0], vec![width, height, source_dims.channels])
        } else {
            Region::rect(x, y, width, height)
        };

        let scale = probe_value_scale(hdu)?;
        let mut data: Vec<f32> = hdu.read_region(&region)?;
        apply_scale(&mut data, scale);
        Ok(Self {
            dims: ImageDims::new(width, height, source_dims.channels),
            data,
            header: hdu.header().clone(),
            scale,
        })
    }

    pub fn dims(&self) -> ImageDims {
        self.dims
    }

    pub fn width(&self) -> usize {
        self.dims.width
    }

    pub fn height(&self) -> usize {
        self.dims.height
    }

    pub fn channels(&self) -> usize {
        self.dims.channels
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    /// The scale applied when this frame was loaded.
    pub fn value_scale(&self) -> ValueScale {
        self.scale
    }

    /// Every sample, channel-major.
    pub fn data(&self) -> &[f32] {
        &self.data
    }

    pub fn data_mut(&mut self) -> &mut [f32] {
        &mut self.data
    }

    pub fn into_data(self) -> Vec<f32> {
        self.data
    }

    /// One channel's samples, row-major.
    pub fn channel(&self, channel: usize) -> &[f32] {
        let n = self.dims.width * self.dims.height;
        &self.data[channel * n..(channel + 1) * n]
    }

    pub fn channel_mut(&mut self, channel: usize) -> &mut [f32] {
        let n = self.dims.width * self.dims.height;
        &mut self.data[channel * n..(channel + 1) * n]
    }

    /// A contiguous run of rows within one channel — the shape of work the
    /// block scheduler hands out.
    pub fn rows(&self, channel: usize, start_row: usize, row_count: usize) -> &[f32] {
        let base = channel * self.dims.width * self.dims.height;
        let start = base + start_row * self.dims.width;
        &self.data[start..start + row_count * self.dims.width]
    }

    #[inline]
    pub fn get(&self, channel: usize, x: usize, y: usize) -> f32 {
        self.data[(channel * self.dims.height + y) * self.dims.width + x]
    }

    #[inline]
    pub fn set(&mut self, channel: usize, x: usize, y: usize, value: f32) {
        self.data[(channel * self.dims.height + y) * self.dims.width + x] = value;
    }

    /// Whether the source declared a Bayer pattern.
    pub fn bayer_pattern(&self) -> Option<String> {
        self.header
            .get_string("BAYERPAT")
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    /// Writes the frame as a 32-bit float FITS image, carrying forward every
    /// non-structural card from the source header.
    ///
    /// `extra` cards are applied after the carried ones, so they win on a
    /// keyword collision — that is how a pipeline stamps `STACKCNT`,
    /// `LIVETIME`, `HISTORY`, and friends onto its output.
    pub fn write(
        &self,
        path: &std::path::Path,
        extra: &[(&str, px_fits::card::Value, Option<&str>)],
    ) -> Result<(), ImageProcError> {
        let mut axes: Vec<u64> = vec![self.dims.width as u64, self.dims.height as u64];
        if self.dims.channels > 1 {
            axes.push(self.dims.channels as u64);
        }

        let mut builder = HeaderBuilder::primary_image(BitPix::F32, &axes)?;
        for card in self.header.cards() {
            // BSCALE/BZERO described the source encoding; this frame is
            // unit-range float and carrying them forward would re-scale it on
            // the next read.
            if is_reserved(&card.keyword) {
                continue;
            }
            // A card the source had but we cannot re-emit (a malformed value,
            // an over-long keyword) is dropped rather than failing the write.
            if let Ok(next) =
                builder
                    .clone()
                    .card(&card.keyword, card.value.clone(), card.comment.as_deref())
            {
                builder = next;
            }
        }
        for (keyword, value, comment) in extra {
            builder = builder.card(keyword, value.clone(), *comment)?;
        }

        let mut writer = FitsWriter::create(path)?;
        writer.write_image(&builder, &self.data)?;
        writer.finish()?;
        Ok(())
    }
}

/// Keywords that must not be carried from a source header onto a written
/// frame: the writer owns the structural ones, and the scaling ones describe
/// an encoding this frame no longer uses.
fn is_reserved(keyword: &str) -> bool {
    let k = keyword.to_ascii_uppercase();
    if matches!(
        k.as_str(),
        "SIMPLE"
            | "BITPIX"
            | "NAXIS"
            | "XTENSION"
            | "END"
            | "PCOUNT"
            | "GCOUNT"
            | "EXTEND"
            | "BSCALE"
            | "BZERO"
            | "BLANK"
    ) {
        return true;
    }
    k.starts_with("NAXIS") && k.len() > 5 && k[5..].chars().all(|c| c.is_ascii_digit())
}

fn dims_of<S: ByteSource + ?Sized>(hdu: &ImageHdu<'_, S>) -> Result<ImageDims, ImageProcError> {
    let shape = hdu.shape();
    match shape {
        [w, h] => Ok(ImageDims::new(*w, *h, 1)),
        [w, h, c] if *c == 1 || *c == 3 => Ok(ImageDims::new(*w, *h, *c)),
        other => Err(ImageProcError::Processing(format!(
            "unsupported image shape {other:?}: expected 2 axes, or 3 with a 1 or 3 element third"
        ))),
    }
}

/// Determines an image's value scale *before* reading its pixels.
///
/// Every caller must use this rather than deciding from whatever data it
/// happens to hold. A block reader that inferred the float scale from its own
/// block would scale different blocks of one frame differently, which is a
/// corrupt frame rather than a slightly wrong one.
///
/// Integer images are settled by the header alone. Float images need a look at
/// the data, so this reads one row from the middle of the frame — one
/// positioned read, independent of frame size. A row that far into the image
/// carries sky background, which is thousands of counts in an unnormalized
/// frame and a small fraction in a normalized one; the gap either side of
/// [`FLOAT_COUNTS_THRESHOLD`] is wide enough that a single row settles it.
pub(crate) fn probe_value_scale<S: ByteSource + ?Sized>(
    hdu: &ImageHdu<'_, S>,
) -> Result<ValueScale, ImageProcError> {
    let bitpix = hdu.bitpix();
    let has_bzero = hdu.scaling().bzero != 0.0;

    if !matches!(bitpix, BitPix::F32 | BitPix::F64) {
        return Ok(ValueScale::choose(bitpix, has_bzero, None));
    }

    let dims = dims_of(hdu)?;
    let row = hdu.read_region::<f32>(&Region::rect(0, dims.height / 2, dims.width, 1))?;
    let max = row.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    Ok(ValueScale::choose(bitpix, has_bzero, Some(max)))
}

/// Applies a scale to a decoded buffer in place.
pub(crate) fn apply_scale(data: &mut [f32], scale: ValueScale) {
    if scale.is_identity() {
        return;
    }
    for v in data.iter_mut() {
        *v = scale.apply(*v);
    }
}
