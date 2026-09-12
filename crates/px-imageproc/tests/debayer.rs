//! Demosaicing tests (ADR 007, Phase 6).
//!
//! Correctness is checked against constructed images whose correct
//! reconstruction is known by inspection, not against another implementation.

use px_imageproc::ImageProcError;
use px_imageproc::bayer::BayerPattern;
use px_imageproc::debayer::{Method, mhc};

/// Colour a CFA site carries under RGGB: 0 red, 1 green, 2 blue.
fn rggb_colour(x: usize, y: usize) -> usize {
    match (y % 2, x % 2) {
        (0, 0) => 0,
        (1, 1) => 2,
        _ => 1,
    }
}

/// Mosaics a full-colour image down to what a Bayer sensor would have
/// measured, so a reconstruction can be compared against the original.
fn mosaic(rgb: &[f32], width: usize, height: usize) -> Vec<f32> {
    let plane = width * height;
    (0..plane)
        .map(|i| {
            let c = rggb_colour(i % width, i / width);
            rgb[c * plane + i]
        })
        .collect()
}

/// Max and RMS error over the interior, skipping the border where the filters
/// fall back to bilinear.
fn interior_error(a: &[f32], b: &[f32], width: usize, height: usize, margin: usize) -> (f32, f32) {
    let plane = width * height;
    let mut max = 0.0f32;
    let mut sum_sq = 0.0f64;
    let mut count = 0usize;
    for c in 0..3 {
        for y in margin..height - margin {
            for x in margin..width - margin {
                let i = c * plane + y * width + x;
                let d = (a[i] - b[i]).abs();
                max = max.max(d);
                sum_sq += (d as f64) * (d as f64);
                count += 1;
            }
        }
    }
    (max, (sum_sq / count as f64).sqrt() as f32)
}

#[test]
fn a_uniform_grey_field_reconstructs_exactly() {
    // Every colour equal everywhere: any correct interpolation must return
    // exactly that, since there is no gradient for the correction term to act
    // on. Catches sign errors in the Laplacian terms, which show up as a
    // uniform field coming back non-uniform.
    let (w, h) = (32usize, 32usize);
    let cfa = vec![0.42f32; w * h];
    let rgb = mhc::demosaic(&cfa, w, h, BayerPattern::Rggb).expect("demosaic");

    for (i, v) in rgb.iter().enumerate() {
        assert!(
            (v - 0.42).abs() < 1e-5,
            "sample {i} came back {v}, expected 0.42"
        );
    }
}

#[test]
fn a_smooth_gradient_reconstructs_closely() {
    // A linear ramp is the case linear interpolation should handle nearly
    // perfectly.
    let (w, h) = (64usize, 64usize);
    let plane = w * h;
    let mut truth = vec![0.0f32; plane * 3];
    for y in 0..h {
        for x in 0..w {
            let v = 0.1 + 0.6 * (x as f32 / w as f32);
            for c in 0..3 {
                truth[c * plane + y * w + x] = v;
            }
        }
    }

    let cfa = mosaic(&truth, w, h);
    let rgb = mhc::demosaic(&cfa, w, h, BayerPattern::Rggb).expect("demosaic");
    let (max, rms) = interior_error(&rgb, &truth, w, h, 2);

    assert!(max < 1e-4, "max error {max} on a linear ramp");
    assert!(rms < 1e-5, "rms error {rms} on a linear ramp");
}

#[test]
fn the_measured_channel_is_never_altered() {
    // At a red site the red value was actually measured. Interpolating over it
    // would throw away the only real data the pixel has.
    let (w, h) = (32usize, 32usize);
    let mut state = 12345u64;
    let cfa: Vec<f32> = (0..w * h)
        .map(|_| {
            state = state.wrapping_mul(6364136223846793005).wrapping_add(1);
            ((state >> 40) as f32) / 16_777_216.0
        })
        .collect();

    let rgb = mhc::demosaic(&cfa, w, h, BayerPattern::Rggb).expect("demosaic");
    let plane = w * h;
    for y in 0..h {
        for x in 0..w {
            let i = y * w + x;
            let c = rggb_colour(x, y);
            assert!(
                (rgb[c * plane + i] - cfa[i]).abs() < 1e-6,
                "measured channel {c} at ({x},{y}) was changed"
            );
        }
    }
}

#[test]
fn a_star_keeps_its_colour_and_its_position() {
    // The case that matters for astrophotography: an isolated bright point on
    // a dark background must come back at the same place, in the same colour,
    // without a ring of the complementary colour around it.
    let (w, h) = (33usize, 33usize);
    let plane = w * h;
    let (cx, cy) = (16usize, 16usize);

    // A small Gaussian, equal in all three channels, so the star is white.
    let mut truth = vec![0.01f32; plane * 3];
    for y in 0..h {
        for x in 0..w {
            let r2 = (x as f32 - cx as f32).powi(2) + (y as f32 - cy as f32).powi(2);
            let v = 0.01 + 0.8 * (-r2 / 4.0).exp();
            for c in 0..3 {
                truth[c * plane + y * w + x] = v;
            }
        }
    }

    let cfa = mosaic(&truth, w, h);
    let rgb = mhc::demosaic(&cfa, w, h, BayerPattern::Rggb).expect("demosaic");

    // The peak is where it was.
    for c in 0..3 {
        let (peak_idx, _) = rgb[c * plane..(c + 1) * plane]
            .iter()
            .enumerate()
            .max_by(|a, b| a.1.total_cmp(b.1))
            .unwrap();
        assert_eq!(
            (peak_idx % w, peak_idx / w),
            (cx, cy),
            "channel {c} peak moved"
        );
    }

    // A white star should stay close to white. It does not stay exactly white:
    // at a red site the red value is measured while green and blue are
    // interpolated, and a linear filter overshoots at a peak this sharp
    // (FWHM ~3.3 px, which is a well-sampled star, not a pathological one).
    // The measured overshoot is ~8.6% of the core value.
    //
    // This is the known limitation of linear gradient-corrected demosaicing
    // and the reason adaptive methods exist. Whether it matters in practice is
    // settled against real frames in tests/debayer_parity.rs, not here.
    let core = cy * w + cx;
    let (r, g, b) = (rgb[core], rgb[plane + core], rgb[2 * plane + core]);
    let spread = r.max(g).max(b) - r.min(g).min(b);
    println!(
        "star core: r={r:.4} g={g:.4} b={b:.4}, spread {:.2}% of green",
        100.0 * spread / g
    );
    assert!(
        spread < 0.12 * g,
        "star core colour spread {:.1}% exceeds what linear demosaicing should produce",
        100.0 * spread / g
    );

    // No channel undershoots the background into negative territory, which is
    // what a ringing artifact around a star looks like.
    let min = rgb.iter().copied().fold(f32::INFINITY, f32::min);
    assert!(min > -0.01, "reconstruction rings below zero: {min}");
}

#[test]
fn every_pattern_puts_the_colours_in_the_right_planes() {
    // A frame where red sites read 1.0 and everything else reads 0 must come
    // back red-dominant, whichever tile orientation the sensor uses.
    let (w, h) = (16usize, 16usize);
    for pattern in [
        BayerPattern::Rggb,
        BayerPattern::Bggr,
        BayerPattern::Gbrg,
        BayerPattern::Grbg,
    ] {
        let tile = match pattern {
            BayerPattern::Rggb => [[0u8, 1], [1, 2]],
            BayerPattern::Bggr => [[2, 1], [1, 0]],
            BayerPattern::Gbrg => [[1, 2], [0, 1]],
            BayerPattern::Grbg => [[1, 0], [2, 1]],
            BayerPattern::None => unreachable!(),
        };
        let cfa: Vec<f32> = (0..w * h)
            .map(|i| {
                let (x, y) = (i % w, i / w);
                if tile[y % 2][x % 2] == 0 { 1.0 } else { 0.0 }
            })
            .collect();

        let rgb = mhc::demosaic(&cfa, w, h, pattern).expect("demosaic");
        let plane = w * h;
        let mean = |c: usize| rgb[c * plane..(c + 1) * plane].iter().sum::<f32>() / plane as f32;

        let (r, g, b) = (mean(0), mean(1), mean(2));
        assert!(
            r > g && r > b,
            "{pattern:?}: red plane should dominate, got r={r} g={g} b={b}"
        );
    }
}

#[test]
fn a_frame_with_no_pattern_is_an_error() {
    let err = mhc::demosaic(&[0.0; 64], 8, 8, BayerPattern::None).unwrap_err();
    assert!(err.to_string().contains("Bayer pattern"), "{err}");
}

#[test]
fn a_frame_too_small_for_the_filters_is_an_error() {
    let err = mhc::demosaic(&[0.0; 16], 4, 4, BayerPattern::Rggb).unwrap_err();
    assert!(err.to_string().contains("at least 5x5"), "{err}");
}

#[test]
fn a_mismatched_buffer_is_an_error() {
    let err = mhc::demosaic(&[0.0; 10], 8, 8, BayerPattern::Rggb).unwrap_err();
    assert!(err.to_string().contains("expected 64"), "{err}");
}

#[test]
fn super_pixel_stays_available_for_previews() {
    // The cheap half-resolution path is still the right tool for a preview,
    // and must not have been disturbed by the new one.
    let (w, h) = (16usize, 16usize);
    let cfa = vec![0.3f32; w * h];
    let (rgb, ow, oh) = px_imageproc::debayer::super_pixel_debayer(&cfa, w, h, BayerPattern::Rggb);
    assert_eq!((ow, oh), (w / 2, h / 2));
    assert_eq!(rgb.len(), ow * oh * 3);
    assert_eq!(Method::default(), Method::GradientCorrected);
}

/// Which reconstruction is actually closer to the truth?
///
/// Comparing two demosaics against each other, or against a third
/// implementation, cannot answer that: a blurrier result measures as
/// *quieter*, because smoothing correlates neighbouring pixels and the
/// background-noise estimator reads that as less noise. The only way to rank
/// them is against an image whose correct answer is known.
///
/// So: build a synthetic star field with known colour and known noise, mosaic
/// it as a Bayer sensor would, reconstruct, and measure the error against what
/// went in.
#[test]
fn reconstruction_error_against_known_truth() {
    let (w, h) = (256usize, 256usize);
    let plane = w * h;

    let mut state = 0x243F_6A88_85A3_08D3u64;
    let mut noise = || {
        state ^= state << 13;
        state ^= state >> 7;
        state ^= state << 17;
        ((state >> 40) as f32 / 16_777_216.0 - 0.5) * 0.004
    };

    // Sky background plus a scattering of stars of assorted brightness and
    // colour, which is what the reconstruction has to cope with.
    let mut truth = vec![0.0f32; plane * 3];
    let stars: Vec<(f32, f32, f32, [f32; 3])> = (0..40)
        .map(|i| {
            let x = 13.0 + (i as f32 * 37.0) % (w as f32 - 26.0);
            let y = 11.0 + (i as f32 * 61.0) % (h as f32 - 22.0);
            let amp = 0.05 + 0.5 * ((i % 7) as f32 / 7.0);
            // Star colours run from blue-white to orange.
            let t = (i % 5) as f32 / 4.0;
            (x, y, amp, [0.6 + 0.4 * t, 1.0, 1.4 - 0.6 * t])
        })
        .collect();

    for y in 0..h {
        for x in 0..w {
            let mut rgb = [0.02f32, 0.022, 0.025];
            for (sx, sy, amp, colour) in &stars {
                let r2 = (x as f32 - sx).powi(2) + (y as f32 - sy).powi(2);
                if r2 < 100.0 {
                    let g = amp * (-r2 / 4.0).exp();
                    for c in 0..3 {
                        rgb[c] += g * colour[c];
                    }
                }
            }
            for c in 0..3 {
                truth[c * plane + y * w + x] = (rgb[c] + noise()).max(0.0);
            }
        }
    }

    let cfa = mosaic(&truth, w, h);

    type Demosaic = fn(&[f32], usize, usize, BayerPattern) -> Result<Vec<f32>, ImageProcError>;
    let candidates: [(&str, Demosaic); 2] = [
        ("bilinear", px_imageproc::debayer::bilinear::demosaic),
        ("gradient-corrected", mhc::demosaic),
    ];

    let mut results = Vec::new();
    for (name, f) in candidates {
        let rgb = f(&cfa, w, h, BayerPattern::Rggb).expect("demosaic");
        let (max, rms) = interior_error(&rgb, &truth, w, h, 3);
        println!("{name:<20} max error {max:.5}  rms error {rms:.6}");
        results.push((name, rms));
    }

    // Whichever wins, both must at least beat treating every missing sample as
    // the sky background — that is the bar for calling it a reconstruction.
    for (name, rms) in &results {
        assert!(
            *rms < 0.02,
            "{name} rms error {rms} is no better than a guess"
        );
    }

    // Record which is closer, so a change that reverses it is visible.
    let (best, best_rms) = results
        .iter()
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .copied()
        .unwrap();
    println!("closest to truth: {best} (rms {best_rms:.6})");
}
