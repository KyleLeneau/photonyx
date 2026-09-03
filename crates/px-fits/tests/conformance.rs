//! Smoke test (ADR 006 O5) for the public `FitsFile` facade: confirms every
//! fixture in the committed corpus opens cleanly through the same API
//! `px-pipeline`/`px` actually call. `tests/header_conformance.rs` and
//! `tests/reader_conformance.rs` exercise the native `Header`/`FitsReader`
//! layers directly and in more depth; this file exists so a bug in the
//! hand-rolled fixture writer (`xtask fits-fixtures`) or in the `FitsFile`
//! compatibility layer itself fails loudly here too.

use std::path::PathBuf;

use px_fits::FitsFile;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

fn valid_fixture_names() -> Vec<&'static str> {
    vec![
        "bitpix8_2d_20x16.fits",
        "bitpix16_2d_64x48.fits",
        "bitpix16_unsigned_bzero_2d_32x32.fits",
        "bitpix32_2d_16x16.fits",
        "bitpix64_1d_100.fits",
        "bitpixneg32_3d_8x8x4.fits",
        "bitpixneg64_2d_10x10.fits",
        "blank_bitpix16_2d_10x10.fits",
        "multi_extension.fits",
        "long_string_continue.fits",
        "header_heavy_4x4.fits",
    ]
}

#[test]
fn every_valid_fixture_opens() {
    for name in valid_fixture_names() {
        let path = fixtures_dir().join(name);
        assert!(
            path.exists(),
            "missing fixture {name}; run `cargo xtask fits-fixtures`"
        );
        let file = FitsFile::new(path.clone());
        assert!(
            file.is_ok(),
            "fixture {name} failed to open: {:?}",
            file.err()
        );
    }
}

#[test]
fn bitpix16_2d_header_reports_expected_axes() {
    let path = fixtures_dir().join("bitpix16_2d_64x48.fits");
    let file = FitsFile::new(path).expect("open fixture");
    let header = file.primary_hdu.get_header();
    assert_eq!(header.naxis().unwrap(), vec![64, 48]);
}

#[test]
fn blank_fixture_declares_blank_keyword() {
    use px_fits::HeaderUtil;

    let path = fixtures_dir().join("blank_bitpix16_2d_10x10.fits");
    let file = FitsFile::new(path).expect("open fixture");
    let header = file.primary_hdu.get_header();
    assert_eq!(header.get_int("BLANK"), Some(-32768));
}

#[test]
fn multi_extension_fixture_has_primary_naxis_zero() {
    let path = fixtures_dir().join("multi_extension.fits");
    let file = FitsFile::new(path).expect("open fixture");
    let header = file.primary_hdu.get_header();
    assert!(header.naxis().unwrap().is_empty());
}

#[test]
fn header_rows_includes_expected_keyword() {
    let path = fixtures_dir().join("bitpix8_2d_20x16.fits");
    let file = FitsFile::new(path).expect("open fixture");
    let rows = file.header_rows();
    assert!(rows.iter().any(|(k, v, _)| k == "OBJECT" && v == "M42"));
}

#[test]
fn is_color_false_for_plain_2d_fixture() {
    let path = fixtures_dir().join("bitpix16_2d_64x48.fits");
    let file = FitsFile::new(path).expect("open fixture");
    assert!(!file.is_color());
}
