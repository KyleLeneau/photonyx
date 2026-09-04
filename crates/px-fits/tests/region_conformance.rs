//! ADR 006 Phase 4 gates: region reads touch only the region's bytes
//! (P4-T4, exact `CountingSource` assertions) and always equal the
//! corresponding crop of a full-frame read (P4-T5), across every fixture.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use px_fits::Region;
use px_fits::header::BitPix;
use px_fits::reader::FitsReader;
use px_fits::source::{ByteSource, FileSource, SliceSource};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Counts bytes and `read_exact_at` calls seen by the inner source (ADR 006
/// O1). Duplicated per integration-test file — they don't share modules.
struct CountingSource<S> {
    inner: S,
    bytes_read: AtomicU64,
    read_calls: AtomicU64,
}

impl<S: ByteSource> CountingSource<S> {
    fn new(inner: S) -> Self {
        Self {
            inner,
            bytes_read: AtomicU64::new(0),
            read_calls: AtomicU64::new(0),
        }
    }
    fn bytes_read(&self) -> u64 {
        self.bytes_read.load(Ordering::Relaxed)
    }
    fn read_calls(&self) -> u64 {
        self.read_calls.load(Ordering::Relaxed)
    }
}

impl<S: ByteSource> ByteSource for CountingSource<S> {
    fn len(&self) -> u64 {
        self.inner.len()
    }
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
        self.inner.read_exact_at(buf, offset)?;
        self.bytes_read
            .fetch_add(buf.len() as u64, Ordering::Relaxed);
        self.read_calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
    // Deliberately no `as_slice` — force the positioned-read path so the
    // byte/call counters actually see the region reads.
}

/// General N-D crop of a row-major (axis-0-fastest) full frame — an
/// independent oracle that does not use the crate's run planner.
fn crop(full: &[f64], dims: &[usize], start: &[usize], shape: &[usize]) -> Vec<f64> {
    let n = dims.len();
    let mut stride = vec![1usize; n];
    for k in 1..n {
        stride[k] = stride[k - 1] * dims[k - 1];
    }
    let count: usize = shape.iter().product();
    let mut out = Vec::with_capacity(count);
    let mut idx = vec![0usize; n];
    for _ in 0..count {
        let lin: usize = (0..n).map(|k| (start[k] + idx[k]) * stride[k]).sum();
        out.push(full[lin]);
        for k in 0..n {
            idx[k] += 1;
            if idx[k] < shape[k] {
                break;
            }
            idx[k] = 0;
        }
    }
    out
}

fn image_fixtures() -> Vec<(&'static str, &'static [usize])> {
    vec![
        ("bitpix8_2d_20x16.fits", &[20, 16]),
        ("bitpix16_2d_64x48.fits", &[64, 48]),
        ("bitpix16_unsigned_bzero_2d_32x32.fits", &[32, 32]),
        ("bitpix32_2d_16x16.fits", &[16, 16]),
        ("bitpix64_1d_100.fits", &[100]),
        ("bitpixneg32_3d_8x8x4.fits", &[8, 8, 4]),
        ("bitpixneg64_2d_10x10.fits", &[10, 10]),
        ("blank_bitpix16_2d_10x10.fits", &[10, 10]),
    ]
}

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| (x.is_nan() && y.is_nan()) || (x - y).abs() <= 1e-9 * x.abs().max(1.0))
}

/// A spread of regions per fixture: corners, single pixel, single row,
/// single column, an interior rectangle, and the full extent.
fn regions_for(dims: &[usize]) -> Vec<Region> {
    let mut out = Vec::new();
    let full_start = vec![0usize; dims.len()];
    out.push(Region::new(full_start.clone(), dims.to_vec())); // full extent

    // single pixel at the far corner
    let last: Vec<usize> = dims.iter().map(|&d| d - 1).collect();
    out.push(Region::new(last.clone(), vec![1usize; dims.len()]));
    // single pixel at the origin
    out.push(Region::new(full_start.clone(), vec![1usize; dims.len()]));

    if dims.len() == 1 {
        out.push(Region::new([dims[0] / 3], [dims[0] / 2]));
        return out;
    }

    // single row (axis 0 full, others pinned mid)
    let mut s = vec![0usize; dims.len()];
    let mut sh = vec![1usize; dims.len()];
    sh[0] = dims[0];
    for k in 1..dims.len() {
        s[k] = dims[k] / 2;
    }
    out.push(Region::new(s, sh));

    // single column (axis 0 pinned, axis 1 full)
    let mut s = vec![0usize; dims.len()];
    let mut sh = vec![1usize; dims.len()];
    s[0] = dims[0] / 2;
    sh[1] = dims[1];
    out.push(Region::new(s, sh));

    // interior rectangle, clamped to bounds
    let s: Vec<usize> = dims
        .iter()
        .map(|&d| (d / 4).min(d.saturating_sub(1)))
        .collect();
    let sh: Vec<usize> = dims
        .iter()
        .zip(&s)
        .map(|(&d, &st)| ((d - st) / 2).max(1))
        .collect();
    out.push(Region::new(s, sh));

    out
}

#[test]
fn region_read_equals_full_frame_crop_for_every_fixture() {
    for (name, dims) in image_fixtures() {
        let path = fixtures_dir().join(name);
        let reader = FitsReader::open(&path).unwrap();
        let img = reader.primary_image().unwrap();
        let full = img.read_full::<f64>().unwrap();

        for region in regions_for(dims) {
            let got = img.read_region::<f64>(&region).unwrap();
            let want = crop(&full, dims, region.start(), region.shape());
            assert!(
                same(&got, &want),
                "{name}: region start={:?} shape={:?} mismatch",
                region.start(),
                region.shape()
            );
        }
    }
}

#[test]
fn out_of_bounds_region_is_rejected_not_clamped() {
    let reader = FitsReader::open(fixtures_dir().join("bitpix16_2d_64x48.fits")).unwrap();
    let img = reader.primary_image().unwrap();

    for bad in [
        Region::rect(60, 0, 8, 4),         // past NAXIS1
        Region::rect(0, 45, 4, 8),         // past NAXIS2
        Region::new([0, 0, 0], [1, 1, 1]), // wrong dimensionality
    ] {
        assert!(
            matches!(
                img.read_region::<i16>(&bad),
                Err(px_fits::FitsError::RegionOutOfBounds(..))
            ),
            "expected RegionOutOfBounds for {bad:?}"
        );
    }
}

#[test]
fn region_read_touches_only_region_bytes_and_one_read_per_row() {
    // 64x48 i16 frame; take a 20x12 window. Expect exactly 12 reads of
    // 20*2 = 40 bytes each = 480 bytes total, and nothing more.
    let path = fixtures_dir().join("bitpix16_2d_64x48.fits");
    let source = CountingSource::new(FileSource::open(&path).unwrap());
    let reader = FitsReader::from_source(source).unwrap();
    let img = reader.primary_image().unwrap();

    let before_bytes = reader.source().bytes_read();
    let before_calls = reader.source().read_calls();

    let region = Region::rect(10, 5, 20, 12);
    let pixels = img.read_region::<i16>(&region).unwrap();
    assert_eq!(pixels.len(), 20 * 12);

    let bytes = reader.source().bytes_read() - before_bytes;
    let calls = reader.source().read_calls() - before_calls;
    assert_eq!(calls, 12, "one positioned read per subset row");
    assert_eq!(bytes, 20 * 12 * 2, "bytes read == region size exactly");
}

#[test]
fn single_row_region_is_one_read() {
    let path = fixtures_dir().join("bitpix32_2d_16x16.fits");
    let source = CountingSource::new(FileSource::open(&path).unwrap());
    let reader = FitsReader::from_source(source).unwrap();
    let img = reader.primary_image().unwrap();

    let before = reader.source().read_calls();
    let _ = img.read_region::<i32>(&Region::rect(0, 7, 16, 1)).unwrap();
    assert_eq!(reader.source().read_calls() - before, 1);
}

#[test]
fn region_matches_read_full_on_a_3d_cube() {
    // Exercises the >2D run planner against a real fixture.
    let path = fixtures_dir().join("bitpixneg32_3d_8x8x4.fits");
    let bytes = std::fs::read(&path).unwrap();
    let reader = FitsReader::from_source(SliceSource::new(bytes)).unwrap();
    let img = reader.primary_image().unwrap();
    assert_eq!(img.bitpix(), BitPix::F32);

    let full = img.read_full::<f64>().unwrap();
    let region = Region::new([1, 2, 0], [4, 3, 2]);
    let got = img.read_region::<f64>(&region).unwrap();
    let want = crop(&full, img.shape(), region.start(), region.shape());
    assert!(same(&got, &want));
}
