//! ADR 006 Phase 3 gate: `ImageHdu::read_full` / `read_full_into` / `rows`
//! against the real fixture corpus, for every `BITPIX`.
//!
//! The oracle is an independent in-test big-endian decoder (`hand_decode`
//! below): it reads the raw data unit straight from the file bytes — using
//! the reader only for the data *offset*, which Phase 2 already validated
//! against `fitsrs` — and applies `BSCALE`/`BZERO`/`BLANK` itself. So this
//! checks the Phase 3 decode path, not the header path, against a
//! second implementation.

use std::path::PathBuf;

use px_fits::header::BitPix;
use px_fits::reader::FitsReader;
use px_fits::source::{FileSource, SliceSource};

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// Every committed valid fixture that carries an image data unit, with its
/// expected geometry.
fn image_fixtures() -> Vec<(&'static str, BitPix, &'static [usize])> {
    vec![
        ("bitpix8_2d_20x16.fits", BitPix::U8, &[20, 16]),
        ("bitpix16_2d_64x48.fits", BitPix::I16, &[64, 48]),
        (
            "bitpix16_unsigned_bzero_2d_32x32.fits",
            BitPix::I16,
            &[32, 32],
        ),
        ("bitpix32_2d_16x16.fits", BitPix::I32, &[16, 16]),
        ("bitpix64_1d_100.fits", BitPix::I64, &[100]),
        ("bitpixneg32_3d_8x8x4.fits", BitPix::F32, &[8, 8, 4]),
        ("bitpixneg64_2d_10x10.fits", BitPix::F64, &[10, 10]),
        ("blank_bitpix16_2d_10x10.fits", BitPix::I16, &[10, 10]),
        ("header_heavy_4x4.fits", BitPix::I16, &[4, 4]),
        ("long_string_continue.fits", BitPix::U8, &[4, 4]),
    ]
}

/// Independent decoder: physical value of every sample as `f64`, NaN where a
/// float target would see one.
fn hand_decode(
    raw: &[u8],
    bitpix: BitPix,
    bscale: f64,
    bzero: f64,
    blank: Option<i64>,
) -> Vec<f64> {
    let n = bitpix.bytes_per_pixel();
    raw.chunks_exact(n)
        .map(|c| match bitpix {
            BitPix::U8 => bzero + bscale * (c[0] as f64),
            BitPix::I16 => {
                let v = i16::from_be_bytes(c.try_into().unwrap()) as i64;
                if blank == Some(v) {
                    f64::NAN
                } else {
                    bzero + bscale * v as f64
                }
            }
            BitPix::I32 => {
                let v = i32::from_be_bytes(c.try_into().unwrap()) as i64;
                if blank == Some(v) {
                    f64::NAN
                } else {
                    bzero + bscale * v as f64
                }
            }
            BitPix::I64 => {
                let v = i64::from_be_bytes(c.try_into().unwrap());
                if blank == Some(v) {
                    f64::NAN
                } else {
                    bzero + bscale * v as f64
                }
            }
            BitPix::F32 => bzero + bscale * (f32::from_be_bytes(c.try_into().unwrap()) as f64),
            BitPix::F64 => bzero + bscale * f64::from_be_bytes(c.try_into().unwrap()),
        })
        .collect()
}

fn same(a: &[f64], b: &[f64]) -> bool {
    a.len() == b.len()
        && a.iter()
            .zip(b)
            .all(|(x, y)| (x.is_nan() && y.is_nan()) || (x - y).abs() <= 1e-9 * x.abs().max(1.0))
}

#[test]
fn read_full_matches_independent_decode_for_every_bitpix() {
    for (name, bitpix, shape) in image_fixtures() {
        let path = fixtures_dir().join(name);
        let bytes = std::fs::read(&path).unwrap_or_else(|_| panic!("read {name}"));

        let reader = FitsReader::from_source(SliceSource::new(bytes.clone())).unwrap();
        let img = reader.primary_image().unwrap();
        assert_eq!(img.bitpix(), bitpix, "{name}: bitpix");
        assert_eq!(img.shape(), shape, "{name}: shape");

        let count: usize = shape.iter().product();
        let data_off = reader.primary().unwrap().data_offset as usize;
        let raw = &bytes[data_off..data_off + count * bitpix.bytes_per_pixel()];

        let s = img.scaling();
        let expected = hand_decode(raw, bitpix, s.bscale, s.bzero, s.blank);
        let got = img.read_full::<f64>().unwrap();
        assert!(same(&got, &expected), "{name}: read_full::<f64> mismatch");
    }
}

#[test]
fn unsigned_bzero_fixture_reads_as_u16_without_touching_floats() {
    let path = fixtures_dir().join("bitpix16_unsigned_bzero_2d_32x32.fits");
    let bytes = std::fs::read(&path).unwrap();
    let reader = FitsReader::from_source(SliceSource::new(bytes.clone())).unwrap();
    let img = reader.primary_image().unwrap();

    let data_off = reader.primary().unwrap().data_offset as usize;
    let raw = &bytes[data_off..data_off + 32 * 32 * 2];
    let expected: Vec<u16> = raw
        .chunks_exact(2)
        .map(|c| (i16::from_be_bytes(c.try_into().unwrap()) as i32 + 32768) as u16)
        .collect();

    assert_eq!(img.read_full::<u16>().unwrap(), expected);
}

#[test]
fn blank_fixture_maps_sentinels_by_target_type() {
    let path = fixtures_dir().join("blank_bitpix16_2d_10x10.fits");
    let reader = FitsReader::open(&path).unwrap();
    let img = reader.primary_image().unwrap();
    assert_eq!(img.scaling().blank, Some(-32768));

    // Float target: BLANK -> NaN, at the indices the fixture forces (0, 5, 17).
    let as_f32 = img.read_full::<f32>().unwrap();
    for i in [0usize, 5, 17] {
        assert!(as_f32[i].is_nan(), "index {i} should be NaN");
    }
    assert_eq!(as_f32.iter().filter(|v| v.is_nan()).count(), 3);

    // Integer target: BLANK passes through as the raw sentinel.
    let as_i16 = img.read_full::<i16>().unwrap();
    for i in [0usize, 5, 17] {
        assert_eq!(as_i16[i], -32768, "index {i} should pass BLANK through");
    }
}

#[test]
fn read_full_into_reuses_caller_buffer() {
    let path = fixtures_dir().join("bitpix32_2d_16x16.fits");
    let reader = FitsReader::open(&path).unwrap();
    let img = reader.primary_image().unwrap();

    let mut buf = vec![0i64; img.len()];
    img.read_full_into(&mut buf).unwrap();
    assert_eq!(buf, img.read_full::<i64>().unwrap());

    let mut wrong = vec![0i64; img.len() + 1];
    assert!(img.read_full_into(&mut wrong).is_err());
}

#[test]
fn rows_stream_reconstructs_full_frame_for_every_fixture() {
    for (name, _bitpix, shape) in image_fixtures() {
        let path = fixtures_dir().join(name);
        let reader = FitsReader::open(&path).unwrap();
        let img = reader.primary_image().unwrap();

        let full = img.read_full::<f64>().unwrap();
        let expected_rows: usize = shape[1..].iter().product();

        let mut stitched = Vec::with_capacity(full.len());
        let mut rows = img.rows::<f64>();
        assert_eq!(rows.rows_left(), expected_rows, "{name}: row count");
        while let Some(row) = rows.next_row() {
            stitched.extend_from_slice(row.unwrap());
        }
        assert!(same(&stitched, &full), "{name}: rows != read_full");
    }
}

#[test]
fn streaming_and_slice_paths_agree_on_a_file_source() {
    // FileSource has no `as_slice`, so this drives the scratch-streaming
    // branch; compare it to the whole-slice branch via SliceSource.
    let path = fixtures_dir().join("bitpix16_2d_64x48.fits");

    let file_reader = FitsReader::from_source(FileSource::open(&path).unwrap()).unwrap();
    let streamed = file_reader
        .primary_image()
        .unwrap()
        .with_scratch(64) // force many refills
        .read_full::<i32>()
        .unwrap();

    let bytes = std::fs::read(&path).unwrap();
    let slice_reader = FitsReader::from_source(SliceSource::new(bytes)).unwrap();
    let sliced = slice_reader
        .primary_image()
        .unwrap()
        .read_full::<i32>()
        .unwrap();

    assert_eq!(streamed, sliced);
}

#[test]
fn image_extension_reads_through_typed_accessor() {
    let path = fixtures_dir().join("multi_extension.fits");
    let reader = FitsReader::open(&path).unwrap();

    // Primary is NAXIS=0: an image HDU with no pixels.
    let primary = reader.primary_image().unwrap();
    assert_eq!(primary.len(), 0);
    assert!(primary.read_full::<u16>().unwrap().is_empty());

    // Extension 1 is a 12x9 BITPIX=16 image.
    let ext = reader.image(1).unwrap();
    assert_eq!(ext.shape(), &[12, 9]);
    assert_eq!(ext.read_full::<i16>().unwrap().len(), 12 * 9);
}

#[test]
fn declared_data_larger_than_file_is_rejected() {
    let path = fixtures_dir().join("invalid/truncated_data.fits");
    let reader = FitsReader::open(&path).unwrap();
    let img = reader.primary_image().unwrap();
    assert!(matches!(
        img.read_full::<i16>(),
        Err(px_fits::FitsError::DataSizeExceedsSource)
    ));
}
