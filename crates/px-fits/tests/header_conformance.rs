//! ADR 006 Phase 1 gate: the native `Header` against the real fixture
//! corpus. Complements `src/header.rs`'s inline-constructed unit tests with
//! the actual bytes `xtask fits-fixtures` produces, and proves the laziness
//! claim (O1) with an exact byte count rather than an assumption.

use std::fs::File;
use std::path::PathBuf;

use px_fits::error::FitsError;
use px_fits::header::{BitPix, Header};
use px_fits::source::{ByteSource, FileSource};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Wraps a `ByteSource` and counts bytes/calls observed through
/// `read_exact_at` — the `ByteSource`-native counterpart to
/// `tests/support.rs`'s `CountingReader` (ADR 006 O1).
struct CountingSource<S> {
    inner: S,
    bytes_read: std::sync::atomic::AtomicU64,
    read_calls: std::sync::atomic::AtomicU64,
}

impl<S: ByteSource> CountingSource<S> {
    fn new(inner: S) -> Self {
        Self {
            inner,
            bytes_read: std::sync::atomic::AtomicU64::new(0),
            read_calls: std::sync::atomic::AtomicU64::new(0),
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
        self.read_calls
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        Ok(())
    }
}

#[test]
fn header_only_scan_touches_only_header_blocks() {
    // header_heavy_4x4.fits has a header inflated to several blocks by
    // padding COMMENT cards, with a tiny data unit — a lazy reader's byte
    // count is dominated by (and exactly equal to) the header blocks.
    let path = fixtures_dir().join("header_heavy_4x4.fits");
    let file_len = std::fs::metadata(&path).unwrap().len();

    let source = CountingSource::new(FileSource::open(&path).unwrap());
    let header = Header::read(&source, 0).unwrap();

    assert_eq!(source.bytes_read(), header.byte_len());
    assert!(
        header.byte_len() < file_len,
        "header ({} bytes) should be smaller than the whole file ({} bytes) for this fixture",
        header.byte_len(),
        file_len,
    );
}

#[test]
fn every_valid_fixture_header_reads_and_validates() {
    let expected: &[(&str, BitPix, &[u64])] = &[
        ("bitpix8_2d_20x16.fits", BitPix::U8, &[20, 16]),
        ("bitpix16_2d_64x48.fits", BitPix::I16, &[64, 48]),
        ("bitpix32_2d_16x16.fits", BitPix::I32, &[16, 16]),
        ("bitpix64_1d_100.fits", BitPix::I64, &[100]),
        ("bitpixneg32_3d_8x8x4.fits", BitPix::F32, &[8, 8, 4]),
        ("bitpixneg64_2d_10x10.fits", BitPix::F64, &[10, 10]),
    ];

    for (name, bitpix, naxis) in expected {
        let path = fixtures_dir().join(name);
        let source = FileSource::open(&path).unwrap();
        let header = Header::read(&source, 0).unwrap();

        assert_eq!(header.bitpix().unwrap(), *bitpix, "{name}: bitpix mismatch");
        assert_eq!(header.naxis().unwrap(), *naxis, "{name}: naxis mismatch");
    }
}

#[test]
fn unsigned_bzero_fixture_header_reads() {
    let path = fixtures_dir().join("bitpix16_unsigned_bzero_2d_32x32.fits");
    let source = FileSource::open(&path).unwrap();
    let header = Header::read(&source, 0).unwrap();

    assert_eq!(header.get_f64("BZERO"), Some(32768.0));
    assert_eq!(header.get_f64("BSCALE"), Some(1.0));
}

#[test]
fn blank_fixture_header_reads() {
    let path = fixtures_dir().join("blank_bitpix16_2d_10x10.fits");
    let source = FileSource::open(&path).unwrap();
    let header = Header::read(&source, 0).unwrap();

    assert_eq!(header.get_i64("BLANK"), Some(-32768));
}

#[test]
fn long_string_fixture_merges_continue_chain() {
    let path = fixtures_dir().join("long_string_continue.fits");
    let source = FileSource::open(&path).unwrap();
    let header = Header::read(&source, 0).unwrap();

    let long = header.get_string("LONGSTR").expect("LONGSTR present");
    assert!(long.starts_with("This is a deliberately long comment-like value"));
    assert!(long.ends_with("exercise the parser."));
    assert!(
        !long.contains('&'),
        "continuation markers must be stripped: {long:?}"
    );
}

#[test]
fn multi_extension_primary_has_zero_axes() {
    let path = fixtures_dir().join("multi_extension.fits");
    let source = FileSource::open(&path).unwrap();
    let header = Header::read(&source, 0).unwrap();

    assert_eq!(header.naxis().unwrap(), Vec::<u64>::new());
}

// --- invalid/ corpus: each fixture's expected outcome (ADR 006 O5) ---

#[test]
fn invalid_missing_end_errors() {
    let path = fixtures_dir().join("invalid/missing_end.fits");
    let source = FileSource::open(&path).unwrap();
    let err = Header::read(&source, 0).unwrap_err();
    assert!(matches!(err, FitsError::MissingEnd));
}

#[test]
fn invalid_invalid_bitpix_header_reads_but_bitpix_accessor_rejects() {
    let path = fixtures_dir().join("invalid/invalid_bitpix.fits");
    let source = FileSource::open(&path).unwrap();
    let header = Header::read(&source, 0).unwrap();
    let err = header.bitpix().unwrap_err();
    assert!(matches!(err, FitsError::InvalidBitpix(3)));
}

#[test]
fn invalid_negative_naxis_header_reads_but_naxis_accessor_rejects() {
    let path = fixtures_dir().join("invalid/negative_naxis.fits");
    let source = FileSource::open(&path).unwrap();
    let header = Header::read(&source, 0).unwrap();
    let err = header.naxis().unwrap_err();
    assert!(matches!(
        err,
        FitsError::NegativeNaxis {
            axis: 1,
            value: -10
        }
    ));
}

#[test]
fn invalid_naxis_overflow_header_reads_but_naxis_accessor_rejects() {
    let path = fixtures_dir().join("invalid/naxis_overflow.fits");
    let source = FileSource::open(&path).unwrap();
    let header = Header::read(&source, 0).unwrap();
    let err = header.naxis().unwrap_err();
    assert!(matches!(err, FitsError::NaxisOverflow));
}

#[test]
fn invalid_bad_card_syntax_header_reads_card_is_invalid_not_an_error() {
    // Permissive parsing (ADR 006 D-series): the malformed card does not
    // fail Header::read. It is queryable and its value is Invalid.
    let path = fixtures_dir().join("invalid/bad_card_syntax.fits");
    let source = FileSource::open(&path).unwrap();
    let header = Header::read(&source, 0).unwrap();
    let card = header
        .cards()
        .iter()
        .find(|c| matches!(c.value, px_fits::card::Value::Invalid(_)));
    assert!(
        card.is_some(),
        "expected at least one Invalid card in this fixture's header"
    );
}

#[test]
fn invalid_truncated_data_header_itself_reads_fine() {
    // The header block in this fixture is well-formed; only the data unit
    // is short. Header::read has no visibility into data-unit length, so it
    // succeeds here by design — the truncation is a Phase 2/3 concern (HDU
    // byte-range bookkeeping / data read), not a Header-parsing concern.
    let path = fixtures_dir().join("invalid/truncated_data.fits");
    let source = FileSource::open(&path).unwrap();
    let header = Header::read(&source, 0).unwrap();
    assert_eq!(header.naxis().unwrap(), vec![100, 100]);

    // Sanity: the file actually is shorter than what NAXIS declares, so a
    // future data-reading phase has something real to catch.
    let declared_data_bytes: u64 = 100 * 100 * 2; // BITPIX=16 -> 2 bytes/px
    let file_len = File::open(&path).unwrap().metadata().unwrap().len();
    assert!(file_len < header.byte_len() + declared_data_bytes);
}
