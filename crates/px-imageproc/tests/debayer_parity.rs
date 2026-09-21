//! Demosaic quality against Siril's RCD, on a real OSC frame
//! (ADR 007, Phases 6 and 6b).
//!
//! The native engine cannot implement RCD — it has no published specification
//! independent of its GPL implementation — so it uses Malvar-He-Cutler
//! instead, with Hirakawa-Parks AHD as the adaptive alternative. This measures
//! what that substitution costs.
//!
//! The comparison is isolated to the demosaic: both sides start from the *same*
//! calibrated CFA frame, the one Siril produced, so calibration differences
//! cannot contaminate the result.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use px_imageproc::Frame;
use px_imageproc::debayer::{Method, debayer_frame};
use px_imageproc::stats::ChannelStats;

const ADU: f64 = 65535.0;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

/// Siril's calibrated CFA frame and its RCD-demosaiced counterpart.
fn references() -> Option<(PathBuf, PathBuf)> {
    let dir = repo_root().join("data/reference-osc/calibrated");
    let entries: Vec<PathBuf> = std::fs::read_dir(&dir)
        .ok()?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .collect();

    let debayered = entries
        .iter()
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("pp_debayered_"))
        })?
        .clone();
    let cfa = entries
        .iter()
        .find(|p| {
            p.file_name()
                .and_then(|n| n.to_str())
                .is_some_and(|n| n.starts_with("pp_") && !n.starts_with("pp_debayered_"))
        })?
        .clone();
    Some((cfa, debayered))
}

/// Reads whole 61 MP frames; seconds optimized, minutes in a debug build.
#[test]
#[cfg_attr(debug_assertions, ignore = "reads 61 MP frames; run with --release")]
fn gradient_corrected_demosaic_compared_against_rcd() {
    let Some((cfa_path, rcd_path)) = references() else {
        eprintln!(
            "skipping: run `cargo xtask siril-reference --corpus data/corpus-osc \
             --out data/reference-osc --calibration`"
        );
        return;
    };

    let theirs = Frame::open(&rcd_path).expect("Siril RCD frame");

    // Both candidates, measured the same way, so the choice between them is
    // made on numbers rather than on reputation.
    for method in [
        Method::Bilinear,
        Method::GradientCorrected,
        Method::Ahd,
        Method::Rcd,
    ] {
        let mut ours = Frame::open(&cfa_path).expect("calibrated CFA frame");
        assert_eq!(
            ours.channels(),
            1,
            "the CFA reference should be one channel"
        );
        let started = std::time::Instant::now();
        debayer_frame(&mut ours, method).expect("demosaic");
        let elapsed = started.elapsed();
        println!("--- {method:?}, {:.2}s ---", elapsed.as_secs_f64());
        for (c, name) in ["red", "green", "blue"].iter().enumerate() {
            let a = ours.channel(c);
            let b = theirs.channel(c);
            let ours_stats = ChannelStats::compute(a, ours.width(), ours.height());
            let theirs_stats = ChannelStats::compute(b, theirs.width(), theirs.height());
            let mut sum_sq = 0.0f64;
            for (x, y) in a.iter().zip(b) {
                let d = (*x as f64 - *y as f64) * ADU;
                sum_sq += d * d;
            }
            println!(
                "  ch{c}: bgnoise ours {:>8.3}  siril {:>8.3}  ratio {:.4}   rms delta {:>8.3} ADU   median {:>8.2} vs {:>8.2}",
                ours_stats.background_noise * ADU,
                theirs_stats.background_noise * ADU,
                ours_stats.background_noise / theirs_stats.background_noise,
                (sum_sq / a.len() as f64).sqrt(),
                ours_stats.median as f64 * ADU,
                theirs_stats.median as f64 * ADU,
            );

            // The gate, for every method on offer: no colour cast. If a
            // channel's level shifted, every frame stacked from it would carry
            // that shift. Everything else here is recorded, not gated — see the
            // note at the foot of this test.
            let shift = (ours_stats.median as f64 - theirs_stats.median as f64).abs()
                / theirs_stats.median as f64;
            assert!(
                shift < 0.01,
                "{method:?}: {name} channel level differs from RCD by {:.3}% \
                 — that is a colour cast",
                shift * 100.0
            );

            // The native RCD is the one method that *is* gated on closeness,
            // because it is the same algorithm as the reference rather than a
            // substitute for it. These bounds are set well outside the measured
            // values (0.93-0.95 and 2.7-3.5 sigma) so they catch a regression
            // rather than pinning the current build; they are deliberately not
            // tight enough to be a fit to Siril's output.
            if method == Method::Rcd {
                let ratio = ours_stats.background_noise / theirs_stats.background_noise;
                assert!(
                    (0.85..=1.15).contains(&ratio),
                    "native RCD {name} background noise is {ratio:.4}x Siril's RCD; \
                     the point of this method is that it matches"
                );
                let sigma =
                    (sum_sq / a.len() as f64).sqrt() / (theirs_stats.background_noise * ADU);
                assert!(
                    sigma < 6.0,
                    "native RCD {name} differs from Siril's RCD by {sigma:.2} sigma_bg"
                );
            }
        }
    }

    let mut ours = Frame::open(&cfa_path).expect("calibrated CFA frame");
    let started = std::time::Instant::now();
    debayer_frame(&mut ours, Method::GradientCorrected).expect("demosaic");
    let elapsed = started.elapsed();

    assert_eq!(
        ours.dims(),
        theirs.dims(),
        "demosaiced frames differ in shape"
    );

    let mut report = String::new();
    let _ = writeln!(
        report,
        "Malvar-He-Cutler (native) vs RCD (Siril 1.4.4)\n  \
         frame {}\n  {}x{}x{}, demosaiced in {:.2}s",
        cfa_path.file_name().unwrap().to_string_lossy(),
        ours.width(),
        ours.height(),
        ours.channels(),
        elapsed.as_secs_f64()
    );

    // Per channel: how far apart are they, and are they equally noisy? The
    // second question is the one that matters — a demosaic that is different
    // but no noisier and no softer is a fine demosaic.
    let mut worst_noise_ratio = 0.0f64;
    for (c, name) in ["red", "green", "blue"].iter().enumerate() {
        let a = ours.channel(c);
        let b = theirs.channel(c);

        let mut max_abs = 0.0f64;
        let mut sum_sq = 0.0f64;
        for (x, y) in a.iter().zip(b) {
            let d = (*x as f64 - *y as f64) * ADU;
            max_abs = max_abs.max(d.abs());
            sum_sq += d * d;
        }
        let rms = (sum_sq / a.len() as f64).sqrt();

        let ours_stats = ChannelStats::compute(a, ours.width(), ours.height());
        let theirs_stats = ChannelStats::compute(b, theirs.width(), theirs.height());
        let noise_ratio = ours_stats.background_noise / theirs_stats.background_noise;
        worst_noise_ratio = worst_noise_ratio.max(noise_ratio);

        let _ = writeln!(
            report,
            "  {name:<6} max |delta| {:>9.2} ADU, rms {:>8.3} ADU ({:.3} sigma_bg)\n  \
             {:<6} median ours {:>9.2}  siril {:>9.2}\n  \
             {:<6} bgnoise ours {:>8.3}  siril {:>8.3}  (ratio {:.4})",
            max_abs,
            rms,
            rms / (theirs_stats.background_noise * ADU),
            "",
            ours_stats.median as f64 * ADU,
            theirs_stats.median as f64 * ADU,
            "",
            ours_stats.background_noise * ADU,
            theirs_stats.background_noise * ADU,
            noise_ratio,
        );
    }

    println!("{report}");

    let out_dir = repo_root().join("data/output/debayer");
    std::fs::create_dir_all(&out_dir).expect("create output dir");
    let out = out_dir.join("native_mhc.fits");
    ours.write(&out, &[]).expect("write demosaiced frame");
    let _ = std::fs::write(repo_root().join("data/output/debayer-parity.txt"), &report);
    println!("written to {}", out.display());

    // What this test gates, and what it only records.
    //
    // It does not gate closeness to RCD. They are different algorithms and
    // differ most exactly where demosaicing is hard — at stars and edges,
    // which is most of an astro frame.
    //
    // It does not gate the background-noise ratio either, and that deserves
    // explaining, because the native result reads noisier. Malvar-He-Cutler is
    // a sharpening reconstruction: its correction term amplifies high spatial
    // frequencies, noise included, and the background estimator sees that as a
    // raised noise floor. RCD averages along the local edge direction and so
    // suppresses it. Measured on this frame the native background reads
    // roughly 1.23x, 1.45x and 1.24x RCD's in red, green and blue.
    //
    // That is not the same as being a worse reconstruction. Bilinear reads
    // *quieter* than RCD on all three channels and is plainly worse — it is
    // just blurrier, and blur correlates neighbouring pixels, which the
    // estimator reads as less noise. Against a synthetic field with known
    // truth (tests/debayer.rs) the gradient-corrected reconstruction is 35%
    // more accurate than bilinear in RMS. Ranking demosaics by their measured
    // background noise rewards the blurriest one.
    //
    // Phase 6b added AHD to find out whether an adaptive method closes that
    // noise gap, since RCD is adaptive and quiet. On this frame it does not:
    // AHD reads 1.20x, 1.55x and 1.32x RCD's background, which is no better
    // than Malvar-He-Cutler and slightly worse in green. It is however
    // markedly *closer* to RCD overall — rms delta 45.2 / 37.2 / 39.7 ADU
    // against MHC's 61.0 / 41.2 / 59.3 — at 7x the time. The reason both sit
    // above RCD is that their directional green carries the same
    // second-derivative sharpening term, and in flat background there is no
    // edge for the homogeneity test to find: it picks between two candidates
    // that are equally noisy.
    //
    // So what is gated is the property that would actually break a pipeline: a
    // colour cast, asserted per method in the loop above.

    // And that the reconstruction is bounded: no channel may run away at a
    // star core, which is what an unstable filter looks like.
    let peak = ours
        .data()
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    let their_peak = theirs
        .data()
        .iter()
        .copied()
        .fold(f32::NEG_INFINITY, f32::max);
    assert!(
        peak <= their_peak * 1.5 + 0.01,
        "native peak {peak} far exceeds RCD's {their_peak}"
    );
}
