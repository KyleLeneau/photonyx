//! Phase 0 smoke test (ADR 006 O5): confirms every fixture in the committed
//! corpus is well-formed FITS by opening it with the current (fitsrs-backed)
//! `FitsFile`. This is not the Phase 1+ conformance suite for the native
//! reader — it exists now purely so a bug in the hand-rolled fixture writer
//! (`xtask fits-fixtures`) fails loudly instead of silently poisoning every
//! later phase's test corpus.

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
    let naxis: Vec<u64> = header.get_xtension().get_naxis().to_vec();
    assert_eq!(naxis, vec![64, 48]);
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
    assert!(header.get_xtension().get_naxis().is_empty());
}
