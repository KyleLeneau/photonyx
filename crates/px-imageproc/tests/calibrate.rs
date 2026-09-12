//! Calibration operator tests (ADR 007, Phase 4).

use std::path::{Path, PathBuf};

use px_fits::card::Value;
use px_fits::reader::FitsReader;
use px_fits::{BitPix, FitsWriter, HeaderBuilder};
use px_imageproc::Frame;
use px_imageproc::bayer::BayerPattern;
use px_imageproc::calibrate::{
    self, CalibrationOptions, DarkScaling, Masters, Thresholds, cosmetic, ops,
};

const U16: f32 = 65535.0;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join("px-imageproc-calibrate-tests")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("create dir");
    d
}

/// Builds a frame from ADU values, with optional header cards.
fn frame_of(
    d: &Path,
    name: &str,
    width: usize,
    height: usize,
    adu: &[u16],
    cards: &[(&str, Value)],
) -> Frame {
    let path = d.join(format!("{name}.fits"));
    let mut header = HeaderBuilder::primary_image(BitPix::I16, &[width as u64, height as u64])
        .expect("header")
        .card("BZERO", Value::Integer(32768), None)
        .expect("bzero");
    for (k, v) in cards {
        header = header.card(k, v.clone(), None).expect("card");
    }
    let stored: Vec<i16> = adu.iter().map(|&v| (v as i32 - 32768) as i16).collect();
    let mut w = FitsWriter::create(&path).expect("create");
    w.write_image(&header, &stored).expect("write");
    w.finish().expect("finish");

    let reader = FitsReader::open(&path).expect("open");
    let hdu = reader.primary_image().expect("image");
    Frame::from_hdu(&hdu).expect("decode")
}

fn flat_of(d: &Path, name: &str, width: usize, height: usize, adu: &[u16]) -> Frame {
    frame_of(d, name, width, height, adu, &[])
}

// ---------------------------------------------------------------------------
// Arithmetic
// ---------------------------------------------------------------------------

#[test]
fn subtraction_keeps_small_negatives_and_clips_garbage() {
    // The rule the whole calibration path depends on: sky noise straddling
    // zero must survive, but a wildly mismatched master must not leave a
    // large negative behind.
    let mut target = vec![0.5, 0.1, 0.0, 0.9];
    let other = vec![0.4, 0.3, 1.5, 0.0];
    let report = ops::subtract_frame(&mut target, &other);

    assert!((target[0] - 0.1).abs() < 1e-6);
    assert!(
        (target[1] - (-0.2)).abs() < 1e-6,
        "small negatives must survive: {target:?}"
    );
    assert_eq!(target[2], 0.0, "-1.5 is garbage, not signal");
    assert!((target[3] - 0.9).abs() < 1e-6);
    assert!((report.negative_ratio() - 0.25).abs() < 1e-6, "{report:?}");
    assert_eq!(
        report.floored, 1,
        "the -1.5 result is destroyed, not merely negative"
    );
    assert_eq!(report.saturated, 0);
}

#[test]
fn subtraction_clamps_the_top_of_the_range() {
    let mut target = vec![0.9];
    let report = ops::subtract_frame(&mut target, &[-0.5]);
    assert_eq!(target[0], 1.0, "a saturated result stays saturated");
    assert_eq!(report.saturated, 1);
}

#[test]
fn the_destructive_clamps_are_counted_separately_from_negatives() {
    // Negatives are preserved and merely counted; the two clamps destroy the
    // value and are counted so a caller can tell the difference.
    let mut target = vec![0.5, 0.5, 0.5, 0.5];
    let report = ops::subtract_frame(&mut target, &[0.4, 0.9, 2.0, -0.9]);

    assert_eq!(report.total, 4);
    assert_eq!(
        report.negative, 1,
        "0.5 - 0.9 = -0.4 survives as a negative"
    );
    assert_eq!(report.floored, 1, "0.5 - 2.0 = -1.5 is past the floor");
    assert_eq!(report.saturated, 1, "0.5 + 0.9 = 1.4 is past the ceiling");
    for (got, want) in target.iter().zip([0.1f32, -0.4, 0.0, 1.0]) {
        assert!((got - want).abs() < 1e-6, "{target:?}");
    }
}

#[test]
fn division_counts_its_clamps_too() {
    // A flat much darker than the level it is normalized to amplifies, which
    // is how a division saturates.
    let mut target = vec![0.5, 0.5];
    let report = ops::divide_frame(&mut target, &[0.01, 0.5], 0.5);
    assert_eq!(report.saturated, 1, "{report:?}");
    assert_eq!(report.floored, 0);
}

#[test]
fn scalar_operations_do_not_clamp() {
    // Interior steps need to go out of range: a dark scaled past 1.0 is then
    // subtracted back down.
    let mut target = vec![0.1, 0.9];
    ops::subtract_scalar(&mut target, 0.5);
    assert!((target[0] - (-0.4)).abs() < 1e-6, "{target:?}");

    let mut target = vec![0.8];
    ops::scale(&mut target, 2.0);
    assert!((target[0] - 1.6).abs() < 1e-6, "{target:?}");
}

#[test]
fn dividing_by_an_empty_flat_pixel_marks_it_absent() {
    // A flat that reads zero somewhere knows nothing about that pixel's
    // response; the alternative to zero is an infinity.
    let mut target = vec![0.5, 0.5];
    ops::divide_frame(&mut target, &[0.0, 0.5], 0.5);
    assert_eq!(target[0], 0.0);
    assert!((target[1] - 0.5).abs() < 1e-6);
}

// ---------------------------------------------------------------------------
// The calibration sequence
// ---------------------------------------------------------------------------

#[test]
fn bias_dark_and_flat_are_applied_in_order() {
    // Constructed so the arithmetic is checkable by hand: light 1000, bias
    // 100, dark 300 (which includes the bias), flat uniform 20000.
    //
    // Siril's masters are built so the dark already contains the bias, so
    // calibrating with both subtracts it twice. That is the documented
    // behaviour, not a bug in the test.
    let d = dir("order");
    let (w, h) = (6usize, 6usize);
    let mut light = frame_of(&d, "light", w, h, &vec![1000u16; w * h], &[]);
    let bias = frame_of(&d, "bias", w, h, &vec![100u16; w * h], &[]);
    let dark = frame_of(&d, "dark", w, h, &vec![300u16; w * h], &[]);
    let flat = flat_of(&d, "flat", w, h, &vec![20000u16; w * h]);

    let masters = Masters {
        bias: Some(&bias),
        dark: Some(&dark),
        flat: Some(&flat),
        ..Default::default()
    };
    let report = calibrate::calibrate(&mut light, &masters, &CalibrationOptions::default())
        .expect("calibrate");

    // (1000 - 100 - 300) / 20000 * 20000 = 600 ADU.
    let got = light.data()[0] * U16;
    assert!((got - 600.0).abs() < 0.05, "got {got} ADU");
    assert_eq!(report.negative_ratio, 0.0);

    let level = report.flat_level.expect("flat level recorded");
    assert!(
        (level * U16 - 20000.0).abs() < 0.05,
        "flat level {}",
        level * U16
    );
}

#[test]
fn a_flat_corrects_vignetting() {
    // A light of uniform sky through optics that dim the edges. After
    // dividing by a flat with the same shape, the sky should be uniform.
    let d = dir("vignette");
    let (w, h) = (32usize, 32usize);

    let response = |x: usize, y: usize| {
        let (cx, cy) = (w as f32 / 2.0, h as f32 / 2.0);
        let r = ((x as f32 - cx).powi(2) + (y as f32 - cy).powi(2)).sqrt();
        1.0 - 0.4 * (r / cx).min(1.0)
    };

    let light_adu: Vec<u16> = (0..w * h)
        .map(|i| (5000.0 * response(i % w, i / w)) as u16)
        .collect();
    let flat_adu: Vec<u16> = (0..w * h)
        .map(|i| (30000.0 * response(i % w, i / w)) as u16)
        .collect();

    let mut light = frame_of(&d, "light", w, h, &light_adu, &[]);
    let flat = flat_of(&d, "flat", w, h, &flat_adu);

    let before: Vec<f32> = light.data().to_vec();
    let spread_before = spread(&before);

    let masters = Masters {
        flat: Some(&flat),
        ..Default::default()
    };
    calibrate::calibrate(&mut light, &masters, &CalibrationOptions::default()).expect("calibrate");

    let spread_after = spread(light.data());
    assert!(
        spread_after < spread_before / 10.0,
        "flat did not flatten: spread {spread_before} -> {spread_after}"
    );
}

fn spread(data: &[f32]) -> f32 {
    let max = data.iter().copied().fold(f32::NEG_INFINITY, f32::max);
    let min = data.iter().copied().fold(f32::INFINITY, f32::min);
    max - min
}

#[test]
fn a_synthetic_bias_level_replaces_a_master() {
    let d = dir("bias-level");
    let (w, h) = (4usize, 4usize);
    let mut light = frame_of(&d, "light", w, h, &vec![1000u16; w * h], &[]);

    let masters = Masters {
        bias_level: Some(500.0 / U16),
        ..Default::default()
    };
    calibrate::calibrate(&mut light, &masters, &CalibrationOptions::default()).expect("calibrate");
    assert!((light.data()[0] * U16 - 500.0).abs() < 0.05);
}

#[test]
fn calibrating_with_no_masters_changes_nothing() {
    let d = dir("nothing");
    let (w, h) = (4usize, 4usize);
    let mut light = frame_of(&d, "light", w, h, &vec![1000u16; w * h], &[]);
    let before = light.data().to_vec();

    let report = calibrate::calibrate(
        &mut light,
        &Masters::default(),
        &CalibrationOptions::default(),
    )
    .expect("calibrate");
    assert_eq!(light.data(), before.as_slice());
    assert_eq!(report, Default::default());
}

#[test]
fn a_mismatched_master_is_reported_with_both_shapes() {
    let d = dir("mismatch");
    let mut light = frame_of(&d, "light", 8, 8, &[1000u16; 64], &[]);
    let bias = frame_of(&d, "bias", 8, 4, &[100u16; 32], &[]);

    let masters = Masters {
        bias: Some(&bias),
        ..Default::default()
    };
    let err =
        calibrate::calibrate(&mut light, &masters, &CalibrationOptions::default()).unwrap_err();
    let msg = err.to_string();
    assert!(msg.contains("8x4x1") && msg.contains("8x8x1"), "{msg}");
}

#[test]
fn an_over_subtracted_frame_reports_its_negative_ratio() {
    let d = dir("negatives");
    let (w, h) = (8usize, 8usize);
    let mut light = frame_of(&d, "light", w, h, &vec![500u16; w * h], &[]);
    let dark = frame_of(&d, "dark", w, h, &vec![900u16; w * h], &[]);

    let masters = Masters {
        dark: Some(&dark),
        ..Default::default()
    };
    let report = calibrate::calibrate(&mut light, &masters, &CalibrationOptions::default())
        .expect("calibrate");

    assert!(
        (report.negative_ratio - 1.0).abs() < 1e-6,
        "every pixel should be negative: {report:?}"
    );
}

// ---------------------------------------------------------------------------
// Dark scaling
// ---------------------------------------------------------------------------

#[test]
fn exposure_scaling_uses_the_ratio_from_the_headers() {
    let d = dir("exposure");
    let (w, h) = (4usize, 4usize);
    let mut light = frame_of(
        &d,
        "light",
        w,
        h,
        &vec![1000u16; w * h],
        &[("EXPTIME", Value::Float(150.0))],
    );
    let dark = frame_of(
        &d,
        "dark",
        w,
        h,
        &vec![400u16; w * h],
        &[("EXPTIME", Value::Float(300.0))],
    );

    let masters = Masters {
        dark: Some(&dark),
        ..Default::default()
    };
    let options = CalibrationOptions {
        dark_scaling: DarkScaling::Exposure,
        ..Default::default()
    };
    let report = calibrate::calibrate(&mut light, &masters, &options).expect("calibrate");

    assert!((report.dark_factor.unwrap() - 0.5).abs() < 1e-6);
    // 1000 - 0.5 * 400 = 800
    assert!((light.data()[0] * U16 - 800.0).abs() < 0.05);
}

#[test]
fn noise_minimization_finds_a_scaled_thermal_signal() {
    // A light carrying exactly 0.5x the dark's thermal pattern plus noise.
    // The search should recover something close to 0.5 — the factor that
    // cancels the pattern and leaves only the noise.
    let d = dir("optimize");
    let (w, h) = (64usize, 64usize);

    let mut state = 0x2545_F491_4F6C_DD1Du64;
    let mut noise = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        ((state >> 40) % 40) as i32 - 20
    };

    // A strong, structured thermal pattern, not just a gradient.
    let thermal = |i: usize| (((i * 37) % 2000) as i32) + (((i / w) * 11) % 500) as i32;

    let dark_adu: Vec<u16> = (0..w * h).map(|i| (500 + thermal(i)) as u16).collect();
    let light_adu: Vec<u16> = (0..w * h)
        .map(|i| (1000 + thermal(i) / 2 + noise()).max(0) as u16)
        .collect();

    let light = frame_of(&d, "light", w, h, &light_adu, &[]);
    let dark = frame_of(&d, "dark", w, h, &dark_adu, &[]);

    let factor = calibrate::dark_optimize::search_optimal_factor(&light, &dark);
    assert!(
        (factor - 0.5).abs() < 0.1,
        "search found {factor}, expected about 0.5"
    );
}

#[test]
fn noise_minimization_on_a_frame_with_no_thermal_signal_is_harmless() {
    let d = dir("optimize-flat");
    let (w, h) = (32usize, 32usize);
    let light = frame_of(&d, "light", w, h, &vec![1000u16; w * h], &[]);
    let dark = frame_of(&d, "dark", w, h, &vec![300u16; w * h], &[]);

    let factor = calibrate::dark_optimize::search_optimal_factor(&light, &dark);
    assert!(
        factor.is_finite() && (0.0..=2.0).contains(&factor),
        "{factor}"
    );
}

// ---------------------------------------------------------------------------
// Cosmetic correction
// ---------------------------------------------------------------------------

#[test]
fn hot_and_cold_pixels_are_found_and_patched() {
    let d = dir("cosmetic");
    let (w, h) = (16usize, 16usize);

    let mut dark_adu = vec![300u16; w * h];
    dark_adu[5 * w + 5] = 60000; // hot
    dark_adu[9 * w + 9] = 0; // cold
    let dark = frame_of(&d, "dark", w, h, &dark_adu, &[]);

    let deviants = cosmetic::find_deviant_pixels(&dark, Thresholds::default_from_dark());
    assert_eq!(
        deviants.hot,
        vec![(5 * w + 5) as u32],
        "hot pixel not found"
    );
    assert!(
        deviants.cold.contains(&((9 * w + 9) as u32)),
        "cold pixel not found: {:?}",
        deviants.cold
    );

    // The same defects in a light frame get patched from their neighbours.
    let mut light_adu = vec![1000u16; w * h];
    light_adu[5 * w + 5] = 60000;
    light_adu[9 * w + 9] = 0;
    let mut light = frame_of(&d, "light", w, h, &light_adu, &[]);

    cosmetic::correct(&mut light, &deviants, false);
    assert!(
        (light.data()[5 * w + 5] * U16 - 1000.0).abs() < 1.0,
        "hot pixel not patched: {}",
        light.data()[5 * w + 5] * U16
    );
    assert!(
        (light.data()[9 * w + 9] * U16 - 1000.0).abs() < 1.0,
        "cold pixel not patched: {}",
        light.data()[9 * w + 9] * U16
    );
}

#[test]
fn cfa_correction_samples_same_coloured_neighbours() {
    // A red pixel surrounded by much brighter green neighbours. Patching it
    // without CFA awareness would pull it toward green and leave a colour
    // artifact exactly where the defect was.
    let d = dir("cosmetic-cfa");
    let (w, h) = (16usize, 16usize);

    let adu_at = |x: usize, y: usize| -> u16 {
        match (y % 2, x % 2) {
            (0, 0) => 1000, // R
            (1, 1) => 1200, // B
            _ => 30000,     // G
        }
    };

    let mut light_adu: Vec<u16> = (0..w * h).map(|i| adu_at(i % w, i / w)).collect();
    // Break a red pixel at an even/even position.
    let defect = 6 * w + 6;
    light_adu[defect] = 60000;
    let mut light = frame_of(&d, "light", w, h, &light_adu, &[]);

    let deviants = cosmetic::DeviantPixels {
        hot: vec![defect as u32],
        cold: Vec::new(),
    };
    cosmetic::correct(&mut light, &deviants, true);

    let patched = light.data()[defect] * U16;
    assert!(
        (patched - 1000.0).abs() < 1.0,
        "CFA-aware patch should give the red level 1000, got {patched}"
    );
}

#[test]
fn cosmetic_correction_without_a_dark_is_an_error() {
    let d = dir("cosmetic-nodark");
    let mut light = frame_of(&d, "light", 4, 4, &[1000u16; 16], &[]);
    let bias = frame_of(&d, "bias", 4, 4, &[100u16; 16], &[]);

    let masters = Masters {
        bias: Some(&bias),
        ..Default::default()
    };
    let options = CalibrationOptions {
        cosmetic: Some(Thresholds::default_from_dark()),
        ..Default::default()
    };
    let err = calibrate::calibrate(&mut light, &masters, &options).unwrap_err();
    assert!(err.to_string().contains("master dark"), "{err}");
}

// ---------------------------------------------------------------------------
// CFA equalization
// ---------------------------------------------------------------------------

#[test]
fn equalizing_a_flat_brings_its_colour_channels_to_one_level() {
    let d = dir("equalize");
    let (w, h) = (16usize, 16usize);

    // A flat where the sensor's red response is low and blue is high.
    let adu_at = |x: usize, y: usize| -> u16 {
        match (y % 2, x % 2) {
            (0, 0) => 10000, // R
            (1, 1) => 30000, // B
            _ => 20000,      // G
        }
    };
    let mut flat = flat_of(
        &d,
        "flat",
        w,
        h,
        &(0..w * h).map(|i| adu_at(i % w, i / w)).collect::<Vec<_>>(),
    );

    calibrate::equalize_cfa(&mut flat, BayerPattern::Rggb).expect("equalize");

    let mean_of = |want: usize| {
        let mut sum = 0.0f64;
        let mut n = 0;
        for y in 0..h {
            for x in 0..w {
                let c = match (y % 2, x % 2) {
                    (0, 0) => 0,
                    (1, 1) => 2,
                    _ => 1,
                };
                if c == want {
                    sum += flat.data()[y * w + x] as f64;
                    n += 1;
                }
            }
        }
        sum / n as f64 * U16 as f64
    };

    let (r, g, b) = (mean_of(0), mean_of(1), mean_of(2));
    assert!((r - g).abs() < 1.0, "red {r} vs green {g}");
    assert!((b - g).abs() < 1.0, "blue {b} vs green {g}");
}

#[test]
fn equalizing_without_a_pattern_is_an_error() {
    let d = dir("equalize-nopattern");
    let mut flat = flat_of(&d, "flat", 4, 4, &[20000u16; 16]);
    let err = calibrate::equalize_cfa(&mut flat, BayerPattern::None).unwrap_err();
    assert!(err.to_string().contains("Bayer pattern"), "{err}");
}
