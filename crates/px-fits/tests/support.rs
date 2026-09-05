//! Shared test-only support (ADR 006 O1, O2): a generic byte-counting reader
//! wrapper and a synthetic-fixture generator, usable from any integration
//! test file via `#[path = "support.rs"] mod support;`.

#![allow(dead_code)]

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

const BLOCK: usize = 2880;

/// Wraps any [`Read`] and records total bytes and call count observed through
/// it. ADR 006 O1: this is how "does this only touch the header blocks?" and
/// "does this read exactly the region requested?" become exact assertions
/// instead of assumptions. Starting in Phase 1 this is paired with
/// `ByteSource` implementations directly; for now it wraps a plain reader so
/// it is usable against any code that accepts `impl Read`.
pub struct CountingReader<R> {
    inner: R,
    bytes_read: usize,
    read_calls: usize,
}

impl<R: Read> CountingReader<R> {
    pub fn new(inner: R) -> Self {
        Self {
            inner,
            bytes_read: 0,
            read_calls: 0,
        }
    }

    pub fn bytes_read(&self) -> usize {
        self.bytes_read
    }

    pub fn read_calls(&self) -> usize {
        self.read_calls
    }
}

impl<R: Read> Read for CountingReader<R> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.bytes_read += n;
        self.read_calls += 1;
        Ok(n)
    }
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

fn card(line: &str) -> [u8; 80] {
    let mut buf = [b' '; 80];
    let bytes = line.as_bytes();
    assert!(bytes.len() <= 80);
    buf[..bytes.len()].copy_from_slice(bytes);
    buf
}

/// Writes a single-HDU, BITPIX=16, 2D synthetic FITS image of `width x height`
/// pixels to `dir`, returning its path. Deterministic content (fixed seed),
/// reused across test runs rather than regenerated when already present.
pub fn write_synthetic_i16_image(dir: &Path, width: usize, height: usize) -> PathBuf {
    std::fs::create_dir_all(dir).expect("create test temp dir");
    let path = dir.join(format!("synthetic_{width}x{height}_i16.fits"));
    if path.exists() {
        return path;
    }

    let mut header = Vec::new();
    header.extend_from_slice(&card(
        "SIMPLE  =                    T / conforms to FITS standard",
    ));
    header.extend_from_slice(&card(
        "BITPIX  =                   16 / bits per data value",
    ));
    header.extend_from_slice(&card("NAXIS   =                    2 / number of axes"));
    header.extend_from_slice(&card(&format!("NAXIS1  = {width:>20} / axis 1 length")));
    header.extend_from_slice(&card(&format!("NAXIS2  = {height:>20} / axis 2 length")));
    header.extend_from_slice(&card("END"));
    let rem = header.len() % BLOCK;
    if rem != 0 {
        header.resize(header.len() + (BLOCK - rem), b' ');
    }

    let mut rng = SplitMix64::new(0xB5_C4_11_E7_00);
    let n = width * height;
    let mut data = Vec::with_capacity(n * 2);
    for _ in 0..n {
        let v = (rng.next_u64() & 0xFFFF) as i16;
        data.extend_from_slice(&v.to_be_bytes());
    }
    let rem = data.len() % BLOCK;
    if rem != 0 {
        data.resize(data.len() + (BLOCK - rem), 0u8);
    }

    let mut f = std::fs::File::create(&path).expect("create synthetic fixture");
    f.write_all(&header).expect("write header");
    f.write_all(&data).expect("write data");
    path
}

pub fn scratch_dir() -> PathBuf {
    std::env::temp_dir().join("px-fits-test-fixtures")
}

pub fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}
