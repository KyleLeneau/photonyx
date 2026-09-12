//! Frame decoding, value normalization, and sequence handling
//! (ADR 007, P1-T1 / P1-T2).

use std::path::{Path, PathBuf};

use px_fits::card::Value;
use px_fits::reader::FitsReader;
use px_fits::{BitPix, FitsWriter, HeaderBuilder};
use px_imageproc::runtime::{Block, ImageDims};
use px_imageproc::{Frame, FrameSet};

const U16: f32 = 65535.0;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join("px-imageproc-frame-tests")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("create test dir");
    d
}

/// Writes an image of `T` with explicit extra cards.
fn write<T: px_fits::Pixel>(
    path: &Path,
    bitpix: BitPix,
    axes: &[u64],
    data: &[T],
    cards: &[(&str, Value)],
) {
    let mut header = HeaderBuilder::primary_image(bitpix, axes).expect("header");
    for (k, v) in cards {
        header = header.card(k, v.clone(), None).expect("card");
    }
    let mut w = FitsWriter::create(path).expect("create");
    w.write_image(&header, data).expect("write");
    w.finish().expect("finish");
}

fn load(path: &Path) -> Frame {
    let reader = FitsReader::open(path).expect("open");
    let hdu = reader.primary_image().expect("primary image");
    Frame::from_hdu(&hdu).expect("decode frame")
}

// ---------------------------------------------------------------------------
// Value normalization
// ---------------------------------------------------------------------------

#[test]
fn unsigned_16_bit_maps_onto_the_unit_range() {
    // The standard astro camera case: BITPIX 16 with BZERO 32768, so the
    // physical values run 0..65535 and only the divide is left to do.
    let d = dir("u16");
    let path = d.join("f.fits");
    let raw: Vec<i16> = vec![-32768, -32768 + 1000, 0, 32767];
    write(
        &path,
        BitPix::I16,
        &[4, 1],
        &raw,
        &[
            ("BZERO", Value::Integer(32768)),
            ("BSCALE", Value::Integer(1)),
        ],
    );

    let frame = load(&path);
    let got = frame.data();
    assert!((got[0] - 0.0).abs() < 1e-6, "{got:?}");
    assert!((got[1] - 1000.0 / U16).abs() < 1e-6, "{got:?}");
    assert!((got[2] - 32768.0 / U16).abs() < 1e-6, "{got:?}");
    assert!((got[3] - 65535.0 / U16).abs() < 1e-6, "{got:?}");
}

#[test]
fn genuinely_signed_16_bit_is_shifted_before_dividing() {
    // No BZERO: the data really does span -32768..32767, and mapping it
    // straight onto [0,1] by dividing would put half the frame negative.
    let d = dir("i16");
    let path = d.join("f.fits");
    write(&path, BitPix::I16, &[3, 1], &[-32768i16, 0, 32767], &[]);

    let frame = load(&path);
    let got = frame.data();
    assert!((got[0] - 0.0).abs() < 1e-6, "{got:?}");
    assert!((got[1] - 32768.0 / U16).abs() < 1e-6, "{got:?}");
    assert!(got.iter().all(|v| (0.0..=1.0).contains(v)), "{got:?}");
}

#[test]
fn eight_bit_divides_by_its_own_full_scale() {
    let d = dir("u8");
    let path = d.join("f.fits");
    write(&path, BitPix::U8, &[3, 1], &[0u8, 128, 255], &[]);

    let frame = load(&path);
    let got = frame.data();
    assert!((got[0] - 0.0).abs() < 1e-6);
    assert!((got[2] - 1.0).abs() < 1e-6, "{got:?}");
}

#[test]
fn float_already_in_the_unit_range_is_left_alone() {
    let d = dir("f32-unit");
    let path = d.join("f.fits");
    // A frame a previous stage produced: sky background around 0.01, stars
    // approaching 1.0. Nothing here looks like raw counts.
    let data: Vec<f32> = (0..64)
        .map(|i| if i == 30 { 0.87 } else { 0.012 })
        .collect();
    write(&path, BitPix::F32, &[8, 8], &data, &[]);

    let frame = load(&path);
    assert!((frame.data()[30] - 0.87).abs() < 1e-6);
    assert!((frame.data()[0] - 0.012).abs() < 1e-6);
}

#[test]
fn float_carrying_raw_counts_is_divided_down() {
    let d = dir("f32-adu");
    let path = d.join("f.fits");
    // Same frame in ADU. The middle row the probe samples carries the sky
    // background, which is what tips the decision.
    let data: Vec<f32> = (0..64)
        .map(|i| if i == 30 { 57000.0 } else { 800.0 })
        .collect();
    write(&path, BitPix::F32, &[8, 8], &data, &[]);

    let frame = load(&path);
    assert!(
        (frame.data()[30] - 57000.0 / U16).abs() < 1e-5,
        "{:?}",
        frame.data()[30]
    );
    assert!((frame.data()[0] - 800.0 / U16).abs() < 1e-5);
}

// ---------------------------------------------------------------------------
// Layout
// ---------------------------------------------------------------------------

#[test]
fn rgb_frames_are_planar_and_channel_major() {
    let d = dir("rgb");
    let path = d.join("f.fits");
    // 2x2x3, channel-major: channel c is filled with (c+1)*1000.
    let mut data: Vec<i16> = Vec::new();
    for c in 0..3i16 {
        data.extend(std::iter::repeat_n((c + 1) * 1000, 4));
    }
    write(
        &path,
        BitPix::I16,
        &[2, 2, 3],
        &data,
        &[("BZERO", Value::Integer(0))],
    );

    let frame = load(&path);
    assert_eq!(frame.dims(), ImageDims::new(2, 2, 3));
    for c in 0..3 {
        let expected = ((c as f32 + 1.0) * 1000.0 + 32768.0) / U16;
        for &v in frame.channel(c) {
            assert!(
                (v - expected).abs() < 1e-6,
                "channel {c}: {v} != {expected}"
            );
        }
    }
    // get() agrees with channel().
    assert_eq!(frame.get(2, 1, 1), frame.channel(2)[3]);
}

#[test]
fn rows_returns_a_contiguous_run_within_one_channel() {
    let d = dir("rows");
    let path = d.join("f.fits");
    let data: Vec<i16> = (0..(4 * 6 * 3) as i16).collect();
    write(&path, BitPix::I16, &[4, 6, 3], &data, &[]);

    let frame = load(&path);
    // Channel 1, rows 2..4 — 8 samples, and they must equal the same slice of
    // that channel.
    let rows = frame.rows(1, 2, 2);
    assert_eq!(rows.len(), 8);
    assert_eq!(rows, &frame.channel(1)[8..16]);
}

// ---------------------------------------------------------------------------
// Write round-trip
// ---------------------------------------------------------------------------

#[test]
fn writing_a_frame_round_trips_its_pixels_and_carries_metadata() {
    let d = dir("roundtrip");
    let src = d.join("src.fits");
    let out = d.join("out.fits");
    write(
        &src,
        BitPix::I16,
        &[8, 8],
        &(0..64i16).map(|i| i * 400).collect::<Vec<_>>(),
        &[
            ("BZERO", Value::Integer(32768)),
            ("OBJECT", Value::String("M31".into())),
            ("EXPTIME", Value::Float(120.0)),
        ],
    );

    let frame = load(&src);
    frame
        .write(&out, &[("STACKCNT", Value::Integer(17), Some("frames"))])
        .expect("write frame");

    let back = load(&out);
    assert_eq!(back.dims(), frame.dims());
    for (a, b) in frame.data().iter().zip(back.data()) {
        assert!((a - b).abs() < 1e-6, "{a} != {b}");
    }

    // Carried from the source, and stamped by the caller.
    assert_eq!(back.header().get_string("OBJECT").as_deref(), Some("M31"));
    assert_eq!(back.header().get_f64("EXPTIME"), Some(120.0));
    assert_eq!(back.header().get_i64("STACKCNT"), Some(17));

    // The source's BZERO described 16-bit storage. Carrying it onto a float
    // frame would re-offset every pixel on the next read.
    assert!(back.header().get_f64("BZERO").is_none_or(|v| v == 0.0));
    assert_eq!(back.header().bitpix().unwrap(), BitPix::F32);
}

#[test]
fn later_cards_win_over_carried_ones() {
    let d = dir("override");
    let src = d.join("src.fits");
    let out = d.join("out.fits");
    write(
        &src,
        BitPix::I16,
        &[4, 4],
        &[0i16; 16],
        &[("FILTER", Value::String("Ha".into()))],
    );

    let frame = load(&src);
    frame
        .write(&out, &[("FILTER", Value::String("OIII".into()), None)])
        .expect("write");

    assert_eq!(
        load(&out).header().get_string("FILTER").as_deref(),
        Some("OIII")
    );
}

// ---------------------------------------------------------------------------
// FrameSet
// ---------------------------------------------------------------------------

fn write_set(d: &Path, count: usize, w: usize, h: usize, c: usize) -> Vec<PathBuf> {
    (0..count)
        .map(|n| {
            let p = d.join(format!("frame_{n:03}.fits"));
            let data: Vec<i16> = (0..(w * h * c))
                .map(|i| ((i * 7 + n * 13) % 30000) as i16)
                .collect();
            let axes: Vec<u64> = if c > 1 {
                vec![w as u64, h as u64, c as u64]
            } else {
                vec![w as u64, h as u64]
            };
            write(
                &p,
                BitPix::I16,
                &axes,
                &data,
                &[("BZERO", Value::Integer(32768))],
            );
            p
        })
        .collect()
}

#[test]
fn an_empty_set_is_rejected() {
    let err = FrameSet::open(&[]).unwrap_err();
    assert!(err.to_string().contains("no frames"), "{err}");
}

#[test]
fn a_mismatched_frame_is_named_in_the_error() {
    let d = dir("mismatch");
    let mut paths = write_set(&d, 2, 8, 8, 1);
    let odd = d.join("odd.fits");
    write(&odd, BitPix::I16, &[8, 9], &[0i16; 72], &[]);
    paths.push(odd.clone());

    let err = FrameSet::open(&paths).unwrap_err();
    let msg = err.to_string();
    assert!(
        msg.contains("odd.fits"),
        "error does not name the frame: {msg}"
    );
    assert!(
        msg.contains("8x9x1"),
        "error does not describe the mismatch: {msg}"
    );
}

#[test]
fn a_set_reports_shared_geometry_and_preserves_order() {
    let d = dir("geometry");
    let paths = write_set(&d, 5, 16, 12, 1);
    let set = FrameSet::open(&paths).expect("open set");

    assert_eq!(set.len(), 5);
    assert_eq!(set.dims(), ImageDims::new(16, 12, 1));
    assert_eq!(set.frame_bytes(), 16 * 12 * 4);
    let got: Vec<_> = set.paths().collect();
    assert_eq!(got, paths.iter().map(|p| p.as_path()).collect::<Vec<_>>());
}

#[test]
fn a_block_read_matches_the_same_rows_of_a_full_read() {
    // The invariant the whole block-scheduled design rests on: reading a
    // block must give byte-identical values to reading the frame and slicing
    // it. Anything that scales a block differently from a full frame — the
    // float-magnitude probe being the obvious risk — breaks here.
    for (w, h, c) in [(16usize, 12usize, 1usize), (16, 12, 3)] {
        let d = dir(&format!("block-{w}x{h}x{c}"));
        let paths = write_set(&d, 3, w, h, c);
        let set = FrameSet::open(&paths).expect("open set");

        for index in 0..set.len() {
            let full = set.load(index).expect("load frame");
            for channel in 0..c {
                let block = Block {
                    channel,
                    start_row: 3,
                    end_row: 7,
                };
                let mut buf = vec![0.0f32; block.pixel_count(&set.dims())];
                set.read_block_into(index, &block, &mut buf)
                    .expect("read block");

                assert_eq!(
                    buf,
                    full.rows(channel, 3, 5),
                    "frame {index} channel {channel}"
                );
            }
        }
    }
}

#[test]
fn a_block_read_rejects_a_wrongly_sized_buffer() {
    let d = dir("block-size");
    let paths = write_set(&d, 1, 8, 8, 1);
    let set = FrameSet::open(&paths).expect("open set");

    let block = Block {
        channel: 0,
        start_row: 0,
        end_row: 1,
    };
    let mut buf = vec![0.0f32; 3];
    let err = set.read_block_into(0, &block, &mut buf).unwrap_err();
    assert!(err.to_string().contains("need 16"), "{err}");
}

#[test]
fn float_frames_scale_consistently_across_blocks() {
    // Direct regression test for the probe: a float frame whose bright pixels
    // sit in one block and faint sky in another must still be scaled by one
    // factor throughout.
    let d = dir("float-blocks");
    let path = d.join("f.fits");
    let (w, h) = (16usize, 16usize);
    let mut data = vec![900.0f32; w * h];
    // Bright row 0 only.
    for v in data.iter_mut().take(w) {
        *v = 40_000.0;
    }
    write(&path, BitPix::F32, &[w as u64, h as u64], &data, &[]);

    let set = FrameSet::open(&[path]).expect("open");
    let full = set.load(0).expect("load");

    for start in [0usize, 8] {
        let block = Block {
            channel: 0,
            start_row: start,
            end_row: start + 7,
        };
        let mut buf = vec![0.0f32; block.pixel_count(&set.dims())];
        set.read_block_into(0, &block, &mut buf)
            .expect("read block");
        assert_eq!(buf, full.rows(0, start, 8), "block at row {start}");
    }

    // And the scale really was the counts one, not the identity.
    assert!(
        (full.data()[0] - 40_000.0 / U16).abs() < 1e-4,
        "{}",
        full.data()[0]
    );
}
