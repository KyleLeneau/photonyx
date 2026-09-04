//! `ImageHdu`: typed read access to an `IMAGE` (or primary) data unit
//! (FITS Standard 4.0 §4.4.1, §7). ADR 006 Phase 3 delivers full-frame
//! reads (`read_full`, `read_full_into`) and the streaming `rows()`
//! iterator; Phase 4 adds cfitsio-style region selection (`read_region`,
//! `read_region_into`).
//!
//! ADR 006 D4: `read_full` makes exactly one output allocation and streams
//! the data unit into it through a small fixed scratch buffer, converting
//! endianness and applying scaling in place — it never materializes the raw
//! byte image and then converts. When the byte source can hand out the whole
//! file (`ByteSource::as_slice`, e.g. `SliceSource`/`MmapSource`), even the
//! scratch buffer is skipped and decoding reads straight from the mapping.
//!
//! ADR 006 D5: `read_region` plans the subset into contiguous element runs
//! (one per subset row for a 2D rectangle) and issues one positioned read
//! per run, so bytes touched scale with the region, not the image.

pub mod pixel;
pub mod region;
pub mod scaling;

pub use pixel::Pixel;
pub use region::Region;
pub use scaling::Scaling;

use rayon::prelude::*;

use crate::error::FitsError;
use crate::hdu::DiscoveredHdu;
use crate::header::{BitPix, Header};
use crate::image::pixel::decode;
use crate::source::ByteSource;

/// Full-frame reads below this many bytes stay single-threaded — `rayon`
/// task coordination is not worth it for small images (ADR 006 P8-T4).
const PARALLEL_MIN_BYTES: usize = 2 * 1024 * 1024;

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
    /// or bounded streaming, each fanned across a `rayon` pool once the read
    /// is large enough to pay for the coordination (ADR 006 D1 / P8-T4).
    fn fill<T: Pixel>(&self, out: &mut [T]) -> Result<(), FitsError> {
        if out.is_empty() {
            return Ok(());
        }
        let bpp = self.bitpix.bytes_per_pixel();
        let total = out.len() * bpp;
        let parallel = total >= PARALLEL_MIN_BYTES && rayon::current_num_threads() > 1;

        // Whole-file slice (`SliceSource`, and `MmapSource` behind the `mmap`
        // feature): decode straight from the borrowed bytes — no scratch,
        // and no allocation even on the parallel path.
        if let Some(all) = self.source.as_slice() {
            let start = self.data_offset as usize;
            let end = start.checked_add(total).ok_or(FitsError::NaxisOverflow)?;
            let bytes = all
                .get(start..end)
                .ok_or(FitsError::DataSizeExceedsSource)?;
            if parallel {
                let task_px = out.len().div_ceil(rayon::current_num_threads()).max(1);
                let (bitpix, scaling) = (self.bitpix, self.scaling);
                return out
                    .par_chunks_mut(task_px)
                    .enumerate()
                    .try_for_each(|(t, task)| {
                        let off = t * task_px * bpp;
                        decode(task, &bytes[off..off + task.len() * bpp], bitpix, &scaling)
                    });
            }
            return decode(out, bytes, self.bitpix, &self.scaling);
        }

        if parallel {
            return self.fill_parallel(out, bpp);
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

    /// Positioned-read fill fanned across `rayon`: one task per worker, each
    /// `pread`ing its own disjoint byte range (safe — `read_exact_at` takes
    /// `&self`, no shared cursor) into a bounded per-task scratch buffer.
    /// Live scratch is `num_threads * scratch_bytes`, still independent of
    /// image size.
    fn fill_parallel<T: Pixel>(&self, out: &mut [T], bpp: usize) -> Result<(), FitsError> {
        let nthreads = rayon::current_num_threads().max(1);
        let task_px = out.len().div_ceil(nthreads).max(1);
        let scratch_px = (self.scratch_bytes / bpp).max(1);
        let (data_offset, bitpix, scaling) = (self.data_offset, self.bitpix, self.scaling);
        let source = self.source;

        out.par_chunks_mut(task_px)
            .enumerate()
            .try_for_each(|(t, task)| -> Result<(), FitsError> {
                let mut buf = vec![0u8; scratch_px.min(task.len()) * bpp];
                let base_px = t * task_px;
                let mut done = 0;
                while done < task.len() {
                    let n = (task.len() - done).min(scratch_px);
                    let b = &mut buf[..n * bpp];
                    source.read_exact_at(b, data_offset + ((base_px + done) * bpp) as u64)?;
                    decode(&mut task[done..done + n], b, bitpix, &scaling)?;
                    done += n;
                }
                Ok(())
            })
    }

    /// Reads a rectangular subset into a freshly allocated `Vec<T>`, in
    /// row-major order with the region's fastest-varying axis first — i.e.
    /// exactly the layout of the corresponding slice of [`read_full`].
    ///
    /// Bytes read scale with the region, not the image (ADR 006 D5): the
    /// plan is one contiguous run per subset row, one positioned read each.
    pub fn read_region<T: Pixel>(&self, region: &Region) -> Result<Vec<T>, FitsError> {
        region.validate(&self.shape)?;
        let mut out = vec![T::from_i64(0); region.len()];
        self.fill_region(region, &mut out)?;
        Ok(out)
    }

    /// Reads a rectangular subset into a caller-owned buffer. `out.len()`
    /// must equal `region.len()`.
    pub fn read_region_into<T: Pixel>(
        &self,
        region: &Region,
        out: &mut [T],
    ) -> Result<(), FitsError> {
        region.validate(&self.shape)?;
        if out.len() != region.len() {
            return Err(FitsError::BufferLenMismatch {
                expected: region.len(),
                got: out.len(),
            });
        }
        self.fill_region(region, out)
    }

    /// Shared body of `read_region` / `read_region_into`. `region` is already
    /// validated against `self.shape`.
    fn fill_region<T: Pixel>(&self, region: &Region, out: &mut [T]) -> Result<(), FitsError> {
        if out.is_empty() {
            return Ok(());
        }
        // The declared data unit must be fully present (same guard as a
        // full read) before we index into it.
        self.check_bounds()?;

        let bpp = self.bitpix.bytes_per_pixel();
        let runs = region.plan_runs(&self.shape);
        let run_len = region.shape()[0];
        let run_bytes = run_len * bpp;
        let (data_offset, bitpix, scaling) = (self.data_offset, self.bitpix, self.scaling);

        // Region reads are syscall-bound, not decode-bound (many small
        // one-row reads), so `rayon` here only adds coordination + per-task
        // allocation overhead — the Phase 8 backend bench measured a ~2.7x
        // regression. Kept serial: runs land contiguously in `out`
        // (`dst_elem == run_index * run_len`), one reused buffer.
        if let Some(all) = self.source.as_slice() {
            return out.chunks_mut(run_len).zip(runs.iter()).try_for_each(
                |(dst, run)| -> Result<(), FitsError> {
                    let start = (data_offset + run.src_elem * bpp as u64) as usize;
                    let bytes = all
                        .get(start..start + run_bytes)
                        .ok_or(FitsError::DataSizeExceedsSource)?;
                    decode(dst, bytes, bitpix, &scaling)
                },
            );
        }

        // One positioned read per run — read calls == run count, bytes read
        // == region size exactly.
        let mut scratch = vec![0u8; run_bytes];
        for (dst, run) in out.chunks_mut(run_len).zip(runs.iter()) {
            self.source
                .read_exact_at(&mut scratch, data_offset + run.src_elem * bpp as u64)?;
            decode(dst, &scratch, bitpix, &scaling)?;
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
    fn read_region_equals_cropped_full_frame_slice_and_stream() {
        // 8x6 i16 image, values 0..48 row-major.
        let raw: Vec<u8> = (0..48i16).flat_map(|v| v.to_be_bytes()).collect();
        let lines = [
            "SIMPLE  =                    T",
            "BITPIX  =                   16",
            "NAXIS   =                    2",
            "NAXIS1  =                    8",
            "NAXIS2  =                    6",
        ];
        let bytes = file(&lines, &raw);
        let region = Region::rect(2, 1, 4, 3); // cols 2..6, rows 1..4

        // Independent crop of the full frame.
        let slice_reader = FitsReader::from_source(SliceSource::new(bytes.clone())).unwrap();
        let full = slice_reader
            .primary_image()
            .unwrap()
            .read_full::<i16>()
            .unwrap();
        let mut want = Vec::new();
        for row in 1..4 {
            for col in 2..6 {
                want.push(full[row * 8 + col]);
            }
        }

        let via_slice = slice_reader
            .primary_image()
            .unwrap()
            .read_region::<i16>(&region)
            .unwrap();
        assert_eq!(via_slice, want);

        let stream_reader = FitsReader::from_source(NoSlice(bytes)).unwrap();
        let via_stream = stream_reader
            .primary_image()
            .unwrap()
            .read_region::<i16>(&region)
            .unwrap();
        assert_eq!(via_stream, want);
    }

    #[test]
    fn read_region_rejects_oob_and_wrong_buffer_len() {
        let raw: Vec<u8> = (0..16i16).flat_map(|v| v.to_be_bytes()).collect();
        let bytes = file(
            &[
                "SIMPLE  =                    T",
                "BITPIX  =                   16",
                "NAXIS   =                    2",
                "NAXIS1  =                    4",
                "NAXIS2  =                    4",
            ],
            &raw,
        );
        let reader = FitsReader::from_source(SliceSource::new(bytes)).unwrap();
        let img = reader.primary_image().unwrap();

        assert!(matches!(
            img.read_region::<i16>(&Region::rect(2, 0, 4, 1)),
            Err(FitsError::RegionOutOfBounds(..))
        ));

        let mut small = [0i16; 3];
        assert!(matches!(
            img.read_region_into::<i16>(&Region::rect(0, 0, 2, 2), &mut small),
            Err(FitsError::BufferLenMismatch {
                expected: 4,
                got: 3
            })
        ));
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
