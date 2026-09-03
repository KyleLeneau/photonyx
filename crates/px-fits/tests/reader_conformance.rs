//! ADR 006 P2-T3: proves `FitsReader` laziness against the real
//! multi-extension fixture with an exact byte count, not an assumption --
//! opening the file and reading the primary header must never touch the
//! extension's header or data blocks.

use std::path::PathBuf;

use px_fits::hdu::HduKind;
use px_fits::reader::FitsReader;
use px_fits::source::{ByteSource, FileSource};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Same pattern as `tests/header_conformance.rs`'s `CountingSource`,
/// duplicated here since integration test binaries don't share modules
/// without an explicit `#[path]` include.
struct CountingSource<S> {
    inner: S,
    bytes_read: std::sync::atomic::AtomicU64,
}

impl<S: ByteSource> CountingSource<S> {
    fn new(inner: S) -> Self {
        Self {
            inner,
            bytes_read: std::sync::atomic::AtomicU64::new(0),
        }
    }

    fn bytes_read(&self) -> u64 {
        self.bytes_read.load(std::sync::atomic::Ordering::Relaxed)
    }
}

impl<S: ByteSource> ByteSource for CountingSource<S> {
    fn len(&self) -> u64 {
        self.inner.len()
    }

    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
        self.inner.read_exact_at(buf, offset)?;
        self.bytes_read
            .fetch_add(buf.len() as u64, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

#[test]
fn opening_multi_extension_file_touches_only_primary_header() {
    let path = fixtures_dir().join("multi_extension.fits");
    let file_len = std::fs::metadata(&path).unwrap().len();

    let source = CountingSource::new(FileSource::open(&path).unwrap());
    let reader = FitsReader::from_source(source).unwrap();

    let primary = reader.primary().unwrap();
    assert_eq!(primary.kind, HduKind::Primary);

    let bytes_touched = reader.source().bytes_read();
    assert_eq!(
        bytes_touched, primary.header_len,
        "opening + reading the primary header must touch exactly its own header blocks"
    );
    assert!(
        bytes_touched < file_len,
        "must not have touched the extension: read {bytes_touched} of {file_len} total bytes"
    );
}

#[test]
fn requesting_extension_then_touches_its_header_too() {
    let path = fixtures_dir().join("multi_extension.fits");

    let source = CountingSource::new(FileSource::open(&path).unwrap());
    let reader = FitsReader::from_source(source).unwrap();

    let primary = reader.primary().unwrap();
    let ext = reader.hdu(1).unwrap();
    assert_eq!(ext.kind, HduKind::Image);

    let bytes_touched = reader.source().bytes_read();
    assert_eq!(bytes_touched, primary.header_len + ext.header_len);
}

#[test]
fn hdu_count_on_multi_extension_fixture_is_two() {
    let path = fixtures_dir().join("multi_extension.fits");
    let reader = FitsReader::open(&path).unwrap();
    assert_eq!(reader.hdu_count().unwrap(), 2);
}
