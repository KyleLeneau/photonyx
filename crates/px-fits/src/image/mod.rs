//! `ImageHdu`: typed read access to an `IMAGE` (or primary) data unit
//! (FITS Standard 4.0 §4.4.1, §7). ADR 006 Phase 3 delivers full-frame
//! reads (`read_full`, `read_full_into`) and the streaming `rows()`
//! iterator; region selection is Phase 4.
//!
//! ADR 006 D4: `read_full` makes exactly one output allocation and streams
//! the data unit into it through a small fixed scratch buffer, converting
//! endianness and applying scaling in place — it never materializes the raw
//! byte image and then converts. When the byte source can hand out the whole
//! file (`ByteSource::as_slice`, e.g. `SliceSource`/`MmapSource`), even the
//! scratch buffer is skipped and decoding reads straight from the mapping.

pub mod pixel;
pub mod scaling;

pub use pixel::Pixel;
pub use scaling::Scaling;

use crate::error::FitsError;
use crate::hdu::DiscoveredHdu;
use crate::header::{BitPix, Header};
use crate::image::pixel::decode;
use crate::source::ByteSource;

/// Default streaming scratch-buffer size (ADR 006 D4). Overridable per HDU
/// with [`ImageHdu::with_scratch`].
pub const DEFAULT_SCRATCH_BYTES: usize = 256 * 1024;

/// One image HDU, bound to the reader's byte source for the duration of the
/// borrow. Holds a clone of the parsed [`Header`] (headers are small) plus
/// the precomputed data-unit geometry, so reads need no further header work.
#[derive(Debug)]
pub struct ImageHdu<'a, S: ByteSource + ?Sized> {
    source: &'a S,
    header: Header,
    bitpix: BitPix,
    scaling: Scaling,
    /// `NAXIS1..NAXISn`, fastest-varying axis first.
    shape: Vec<usize>,
    /// Byte offset of the data unit within the source.
    data_offset: u64,
    /// Logical (unpadded) data-unit length in bytes.
    data_len: u64,
    scratch_bytes: usize,
}

impl<'a, S: ByteSource + ?Sized> ImageHdu<'a, S> {
    /// Builds an `ImageHdu` from an already-discovered HDU. Validates
    /// `BITPIX` and `NAXISn` eagerly so every later read is infallible on
    /// those grounds.
    pub(crate) fn from_discovered(source: &'a S, hdu: DiscoveredHdu) -> Result<Self, FitsError> {
        let bitpix = hdu.header.bitpix()?;
        let shape: Vec<usize> = hdu
            .header
            .naxis()?
            .into_iter()
            .map(|n| n as usize)
            .collect();
        let scaling = Scaling::from_header(&hdu.header);
        Ok(Self {
            source,
            header: hdu.header,
            bitpix,
            scaling,
            shape,
            data_offset: hdu.data_offset,
            data_len: hdu.data_len,
            scratch_bytes: DEFAULT_SCRATCH_BYTES,
        })
    }

    /// Sets the streaming scratch-buffer size (bytes) used by
    /// [`read_full`](Self::read_full) / [`read_full_into`](Self::read_full_into)
    /// / [`rows`](Self::rows) when the source cannot expose a whole-file
    /// slice. Clamped up to one pixel minimum.
    #[must_use]
    pub fn with_scratch(mut self, bytes: usize) -> Self {
        self.scratch_bytes = bytes.max(self.bitpix.bytes_per_pixel());
        self
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    /// `NAXIS1..NAXISn`, fastest-varying axis first.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    pub fn bitpix(&self) -> BitPix {
        self.bitpix
    }

    pub fn scaling(&self) -> &Scaling {
        &self.scaling
    }

    /// Total pixel count (product of `shape`). Zero for a data-less HDU
    /// (`NAXIS = 0` or any `NAXISn = 0`).
    pub fn len(&self) -> usize {
        if self.shape.is_empty() {
            0
        } else {
            self.shape.iter().product()
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Length in bytes of one row (the fastest-varying axis).
    fn row_bytes(&self) -> usize {
        self.shape.first().copied().unwrap_or(0) * self.bitpix.bytes_per_pixel()
    }

    /// Guards that the declared data unit actually fits in the source before
    /// any buffer is sized from it (ADR 006 O6 allocation guard).
    fn check_bounds(&self) -> Result<u64, FitsError> {
        let need = (self.len() as u64)
            .checked_mul(self.bitpix.bytes_per_pixel() as u64)
            .ok_or(FitsError::NaxisOverflow)?;
        // `data_len` was computed from the same header during discovery; for
        // an image (PCOUNT 0, GCOUNT 1) the two must agree.
        debug_assert_eq!(need, self.data_len);
        let end = self
            .data_offset
            .checked_add(need)
            .ok_or(FitsError::NaxisOverflow)?;
        if end > self.source.len() {
            return Err(FitsError::DataSizeExceedsSource);
        }
        Ok(need)
    }

    /// Reads the whole data unit into a freshly allocated `Vec<T>`, decoding
    /// big-endian samples and applying `BSCALE`/`BZERO`/`BLANK`.
    ///
    /// Exactly one allocation for the result; peak heap is
    /// `len() * size_of::<T>()` plus at most `scratch_bytes` (ADR 006 D4,
    /// performance-targets table).
    pub fn read_full<T: Pixel>(&self) -> Result<Vec<T>, FitsError> {
        let count = self.len();
        self.check_bounds()?;
        let mut out = vec![T::from_i64(0); count];
        self.fill(&mut out)?;
        Ok(out)
    }

    /// Reads the whole data unit into a caller-owned buffer. `out.len()`
    /// must equal [`len()`](Self::len).
    ///
    /// When the source exposes [`ByteSource::as_slice`] this allocates
    /// nothing at all; otherwise it allocates a single bounded scratch
    /// buffer (≤ `scratch_bytes`).
    pub fn read_full_into<T: Pixel>(&self, out: &mut [T]) -> Result<(), FitsError> {
        let count = self.len();
        if out.len() != count {
            return Err(FitsError::BufferLenMismatch {
                expected: count,
                got: out.len(),
            });
        }
        self.check_bounds()?;
        self.fill(out)
    }

    /// Shared body of `read_full` / `read_full_into`: whole-slice fast path
    /// or bounded streaming.
    fn fill<T: Pixel>(&self, out: &mut [T]) -> Result<(), FitsError> {
        if out.is_empty() {
            return Ok(());
        }
        let bpp = self.bitpix.bytes_per_pixel();
        let total = out.len() * bpp;

        if let Some(all) = self.source.as_slice() {
            let start = self.data_offset as usize;
            let end = start.checked_add(total).ok_or(FitsError::NaxisOverflow)?;
            let bytes = all
                .get(start..end)
                .ok_or(FitsError::DataSizeExceedsSource)?;
            return decode(out, bytes, self.bitpix, &self.scaling);
        }

        let scratch_pixels = (self.scratch_bytes / bpp).max(1);
        let mut scratch = vec![0u8; scratch_pixels * bpp];
        let mut offset = self.data_offset;
        let mut done = 0;
        while done < out.len() {
            let this = (out.len() - done).min(scratch_pixels);
            let buf = &mut scratch[..this * bpp];
            self.source.read_exact_at(buf, offset)?;
            decode(&mut out[done..done + this], buf, self.bitpix, &self.scaling)?;
            offset += (this * bpp) as u64;
            done += this;
        }
        Ok(())
    }

    /// A streaming iterator over image rows (the fastest-varying axis),
    /// reusing one raw-read buffer and one decoded-row buffer for the whole
    /// traversal — peak heap is one row plus scratch, independent of image
    /// height (ADR 006 performance-targets table).
    ///
    /// It is a *lending* iterator: each row borrows the internal buffer, so
    /// it cannot implement `std::iter::Iterator` without allocating per row.
    /// Drive it with [`RowIter::next_row`].
    pub fn rows<T: Pixel>(&self) -> RowIter<'_, 'a, S, T> {
        let row_pixels = self.shape.first().copied().unwrap_or(0);
        let total_rows = if self.shape.is_empty() || row_pixels == 0 {
            0
        } else {
            self.shape[1..].iter().product()
        };
        RowIter {
            image: self,
            row_pixels,
            rows_left: total_rows,
            next_offset: self.data_offset,
            raw: vec![0u8; self.row_bytes()],
            row: vec![T::from_i64(0); row_pixels],
        }
    }
}

/// Lending iterator produced by [`ImageHdu::rows`]. See that method's docs.
pub struct RowIter<'img, 'src, S: ByteSource + ?Sized, T: Pixel> {
    image: &'img ImageHdu<'src, S>,
    row_pixels: usize,
    rows_left: usize,
    next_offset: u64,
    raw: Vec<u8>,
    row: Vec<T>,
}

impl<S: ByteSource + ?Sized, T: Pixel> RowIter<'_, '_, S, T> {
    /// Number of rows not yet yielded.
    pub fn rows_left(&self) -> usize {
        self.rows_left
    }

    /// Decodes and returns the next row, or `None` once every row has been
    /// yielded. The returned slice is valid only until the next call.
    pub fn next_row(&mut self) -> Option<Result<&[T], FitsError>> {
        if self.rows_left == 0 {
            return None;
        }
        if let Err(e) = self
            .image
            .source
            .read_exact_at(&mut self.raw, self.next_offset)
        {
            self.rows_left = 0;
            return Some(Err(e.into()));
        }
        if let Err(e) = decode(
            &mut self.row,
            &self.raw,
            self.image.bitpix,
            &self.image.scaling,
        ) {
            self.rows_left = 0;
            return Some(Err(e));
        }
        self.next_offset += self.raw.len() as u64;
        self.rows_left -= 1;
        Some(Ok(&self.row[..self.row_pixels]))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::FitsReader;
    use crate::source::SliceSource;

    const BLOCK: usize = crate::block::BLOCK_SIZE;

    fn card(line: &str) -> Vec<u8> {
        let mut b = line.as_bytes().to_vec();
        assert!(b.len() <= 80);
        b.resize(80, b' ');
        b
    }

    /// Assembles a single-HDU primary file from header lines + raw data bytes.
    fn file(header_lines: &[&str], data: &[u8]) -> Vec<u8> {
        let mut out = Vec::new();
        for line in header_lines {
            out.extend_from_slice(&card(line));
        }
        out.extend_from_slice(&card("END"));
        if out.len() % BLOCK != 0 {
            out.resize(out.len() + (BLOCK - out.len() % BLOCK), b' ');
        }
        out.extend_from_slice(data);
        if out.len() % BLOCK != 0 {
            out.resize(out.len() + (BLOCK - out.len() % BLOCK), 0);
        }
        out
    }

    #[test]
    fn read_full_i16_2d_matches_hand_decode() {
        let raw: Vec<u8> = (0..12i16).flat_map(|v| v.to_be_bytes()).collect();
        let bytes = file(
            &[
                "SIMPLE  =                    T",
                "BITPIX  =                   16",
                "NAXIS   =                    2",
                "NAXIS1  =                    4",
                "NAXIS2  =                    3",
            ],
            &raw,
        );
        let reader = FitsReader::from_source(SliceSource::new(bytes)).unwrap();
        let img = reader.primary_image().unwrap();
        assert_eq!(img.shape(), &[4, 3]);
        assert_eq!(img.len(), 12);
        assert_eq!(img.read_full::<i16>().unwrap(), (0..12).collect::<Vec<_>>());
    }

    #[test]
    fn read_full_applies_bzero_unsigned_fast_path() {
        let raw: Vec<u8> = [-32768i16, -1, 0, 32767]
            .iter()
            .flat_map(|v| v.to_be_bytes())
            .collect();
        let bytes = file(
            &[
                "SIMPLE  =                    T",
                "BITPIX  =                   16",
                "NAXIS   =                    2",
                "NAXIS1  =                    4",
                "NAXIS2  =                    1",
                "BZERO   =                32768",
                "BSCALE  =                    1",
            ],
            &raw,
        );
        let reader = FitsReader::from_source(SliceSource::new(bytes)).unwrap();
        let img = reader.primary_image().unwrap();
        assert_eq!(
            img.read_full::<u16>().unwrap(),
            vec![0, 32767, 32768, 65535]
        );
    }

    #[test]
    fn read_full_into_rejects_wrong_length() {
        let bytes = file(
            &[
                "SIMPLE  =                    T",
                "BITPIX  =                    8",
                "NAXIS   =                    1",
                "NAXIS1  =                    4",
            ],
            &[1, 2, 3, 4],
        );
        let reader = FitsReader::from_source(SliceSource::new(bytes)).unwrap();
        let img = reader.primary_image().unwrap();
        let mut out = [0u8; 3];
        assert!(matches!(
            img.read_full_into(&mut out).unwrap_err(),
            FitsError::BufferLenMismatch {
                expected: 4,
                got: 3
            }
        ));
    }

    #[test]
    fn streaming_path_matches_slice_path() {
        // 5000 i16 pixels forces several scratch refills at a tiny scratch size.
        let raw: Vec<u8> = (0..5000i16)
            .map(|v| v.wrapping_mul(7))
            .flat_map(|v| v.to_be_bytes())
            .collect();
        let bytes = file(
            &[
                "SIMPLE  =                    T",
                "BITPIX  =                   16",
                "NAXIS   =                    2",
                "NAXIS1  =                  100",
                "NAXIS2  =                   50",
            ],
            &raw,
        );

        let slice_reader = FitsReader::from_source(SliceSource::new(bytes.clone())).unwrap();
        let expected = slice_reader
            .primary_image()
            .unwrap()
            .read_full::<i32>()
            .unwrap();

        // NoSliceSource hides `as_slice`, forcing the streaming branch.
        let stream_reader = FitsReader::from_source(NoSlice(bytes)).unwrap();
        let got = stream_reader
            .primary_image()
            .unwrap()
            .with_scratch(512)
            .read_full::<i32>()
            .unwrap();
        assert_eq!(got, expected);
    }

    #[test]
    fn rows_concatenate_to_full_frame() {
        let raw: Vec<u8> = (0..24i16).flat_map(|v| v.to_be_bytes()).collect();
        let bytes = file(
            &[
                "SIMPLE  =                    T",
                "BITPIX  =                   16",
                "NAXIS   =                    2",
                "NAXIS1  =                    6",
                "NAXIS2  =                    4",
            ],
            &raw,
        );
        let reader = FitsReader::from_source(NoSlice(bytes)).unwrap();
        let img = reader.primary_image().unwrap();
        let full = img.read_full::<i16>().unwrap();

        let mut stitched = Vec::new();
        let mut rows = img.rows::<i16>();
        assert_eq!(rows.rows_left(), 4);
        while let Some(row) = rows.next_row() {
            stitched.extend_from_slice(row.unwrap());
        }
        assert_eq!(stitched, full);
    }

    #[test]
    fn empty_primary_reads_as_empty() {
        let mut bytes = Vec::new();
        for line in [
            "SIMPLE  =                    T",
            "BITPIX  =                    8",
            "NAXIS   =                    0",
        ] {
            bytes.extend_from_slice(&card(line));
        }
        bytes.extend_from_slice(&card("END"));
        bytes.resize(BLOCK, b' ');
        let reader = FitsReader::from_source(SliceSource::new(bytes)).unwrap();
        let img = reader.primary_image().unwrap();
        assert_eq!(img.len(), 0);
        assert!(img.read_full::<f32>().unwrap().is_empty());
    }

    /// A `ByteSource` that refuses to expose a whole-file slice, so tests can
    /// exercise the streaming decode branch even from in-memory bytes.
    struct NoSlice(Vec<u8>);

    impl ByteSource for NoSlice {
        fn len(&self) -> u64 {
            self.0.len() as u64
        }
        fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
            let start = offset as usize;
            let end = start + buf.len();
            if end > self.0.len() {
                return Err(std::io::Error::new(
                    std::io::ErrorKind::UnexpectedEof,
                    "past end",
                ));
            }
            buf.copy_from_slice(&self.0[start..end]);
            Ok(())
        }
    }
}
