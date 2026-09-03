//! ADR 006 O3 / Phase 2 gate: differential test against `fitsrs`, the
//! library `px-fits` is replacing. `fitsrs` is a dev-dependency only from
//! this point on -- this file (and that dependency) is deleted at the end
//! of Phase 3 once the native reader has proven itself across the whole
//! corpus and has its own image-reading tests to stand on.
//!
//! Only "real" keyword cards are compared (not COMMENT/HISTORY/blank/END):
//! those are free text with no standardized value/comment split, and
//! comparing their exact internal representation between two independently
//! written parsers is fragile without being informative. Every card that
//! carries an actual value is compared for both value and comment.

use std::collections::BTreeMap;
use std::path::PathBuf;

use fitsrs::card::Value as FValue;
use fitsrs::{Fits, HDU};
use px_fits::card::Value as NValue;
use px_fits::header::Header as NativeHeader;
use px_fits::source::FileSource;

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
        "long_string_continue.fits",
        "header_heavy_4x4.fits",
    ]
    // multi_extension.fits is excluded: its primary has NAXIS=0, which
    // fitsrs's HDU::Primary(Image) path is not built to represent the same
    // way ours is, and there is nothing value-bearing to differentially
    // compare on an empty header beyond what the other fixtures cover.
}

/// A (value, comment) pair in a common, parser-independent representation.
fn fitsrs_row(value: &FValue) -> (String, String) {
    let comment_of = |c: &Option<String>| c.as_deref().unwrap_or("").trim().to_string();
    match value {
        FValue::Integer { value, comment } => (value.to_string(), comment_of(comment)),
        FValue::Float { value, comment } => (format!("{value}"), comment_of(comment)),
        FValue::Logical { value, comment } => (
            if *value { "T" } else { "F" }.to_string(),
            comment_of(comment),
        ),
        FValue::String { value, comment } => (value.trim_end().to_string(), comment_of(comment)),
        FValue::Undefined => ("undefined".to_string(), String::new()),
        FValue::Invalid(raw) => (raw.clone(), String::new()),
    }
}

fn native_row(value: &NValue, comment: &Option<String>) -> (String, String) {
    let val_str = match value {
        NValue::Integer(i) => i.to_string(),
        NValue::Float(f) => format!("{f}"),
        NValue::Logical(b) => (if *b { "T" } else { "F" }).to_string(),
        NValue::String(s) => s.clone(),
        NValue::Complex(re, im) => format!("({re}, {im})"),
        NValue::Undefined => "undefined".to_string(),
        NValue::Commentary(_) => unreachable!("commentary keywords are excluded"),
        NValue::Invalid(raw) => raw.clone(),
    };
    (val_str, comment.clone().unwrap_or_default())
}

fn is_commentary_keyword(k: &str) -> bool {
    matches!(
        k.to_ascii_uppercase().as_str(),
        "COMMENT" | "HISTORY" | "END" | "" | "CONTINUE"
    )
}

#[test]
fn native_header_matches_fitsrs_for_every_fixture() {
    for name in valid_fixture_names() {
        let path = fixtures_dir().join(name);

        // fitsrs side.
        let file = std::fs::File::open(&path).unwrap();
        let reader = std::io::BufReader::new(file);
        let mut fits_reader = Fits::from_reader(reader);
        let primary = match fits_reader
            .next()
            .unwrap_or_else(|| panic!("{name}: no primary HDU"))
            .unwrap()
        {
            HDU::Primary(img) => img,
            _ => panic!("{name}: expected primary HDU"),
        };
        let fitsrs_rows: BTreeMap<String, (String, String)> = primary
            .get_header()
            .iter()
            .filter(|(k, _)| !is_commentary_keyword(k))
            .map(|(k, v)| (k.to_string(), fitsrs_row(v)))
            .collect();

        // native side.
        let source = FileSource::open(&path).unwrap();
        let native_header = NativeHeader::read(&source, 0).unwrap();
        let native_rows: BTreeMap<String, (String, String)> = native_header
            .cards()
            .iter()
            .filter(|c| !is_commentary_keyword(&c.keyword))
            .map(|c| (c.keyword.clone(), native_row(&c.value, &c.comment)))
            .collect();

        assert_eq!(
            native_rows, fitsrs_rows,
            "{name}: native and fitsrs header rows differ"
        );
    }
}
