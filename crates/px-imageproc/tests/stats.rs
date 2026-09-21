//! Statistics tests (ADR 007, P2-T4 / P2-T5).
//!
//! Estimators are checked against analytically known distributions rather
//! than against a reference implementation's output, so a failure points at
//! the estimator rather than at a disagreement.

use px_imageproc::runtime::{ResourceBudget, Runtime};
use px_imageproc::stats::{
    self, ChannelStats, background_noise, biweight_midvariance, location_scale, mad, mean_sigma,
    median, median_in_place, min_max, non_null,
};

/// Deterministic standard-normal samples via Box-Muller over a SplitMix64
/// stream, so every test sees the same frame on every machine.
struct Gauss {
    state: u64,
    spare: Option<f64>,
}

impl Gauss {
    fn new(seed: u64) -> Self {
        Self {
            state: seed,
            spare: None,
        }
    }

    fn uniform(&mut self) -> f64 {
        self.state = self.state.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.state;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        // Open interval: log(0) is not useful to us.
        ((z >> 11) as f64 + 0.5) / (1u64 << 53) as f64
    }

    fn next(&mut self) -> f64 {
        if let Some(v) = self.spare.take() {
            return v;
        }
        let (u1, u2) = (self.uniform(), self.uniform());
        let r = (-2.0 * u1.ln()).sqrt();
        let theta = std::f64::consts::TAU * u2;
        self.spare = Some(r * theta.sin());
        r * theta.cos()
    }
}

/// A flat field of `mean` with Gaussian noise of the given sigma.
fn noisy_field(width: usize, height: usize, mean: f32, sigma: f32, seed: u64) -> Vec<f32> {
    let mut g = Gauss::new(seed);
    (0..width * height)
        .map(|_| mean + sigma * g.next() as f32)
        .collect()
}

// ---------------------------------------------------------------------------
// Basic estimators
// ---------------------------------------------------------------------------

#[test]
fn mean_and_sigma_recover_a_known_distribution() {
    let data = noisy_field(256, 256, 0.25, 0.01, 7);
    let (count, mean, sigma) = mean_sigma(&data);

    assert_eq!(count, 256 * 256);
    assert!((mean - 0.25).abs() < 2e-4, "mean {mean}");
    assert!((sigma - 0.01).abs() < 2e-4, "sigma {sigma}");
}

#[test]
fn sigma_is_the_population_rms() {
    // Four samples, mean 2.5. Population sigma is sqrt(1.25); the sample
    // (n-1) form would give sqrt(5/3) ≈ 1.29.
    let (n, mean, sigma) = mean_sigma(&[1.0, 2.0, 3.0, 4.0]);
    assert_eq!(n, 4);
    assert!((mean - 2.5).abs() < 1e-9);
    assert!((sigma - 1.25f64.sqrt()).abs() < 1e-6, "sigma {sigma}");
}

#[test]
fn zeros_and_nans_are_treated_as_absent() {
    // Half the frame empty must not halve the background estimate.
    let mut data = vec![0.5f32; 100];
    for v in data.iter_mut().take(40) {
        *v = 0.0;
    }
    data[95] = f32::NAN;

    let (count, mean, sigma) = mean_sigma(&data);
    assert_eq!(count, 59, "59 samples carry data");
    assert!((mean - 0.5).abs() < 1e-6, "mean {mean}");
    assert!(sigma < 1e-6);

    assert_eq!(non_null(&data).len(), 59);
    assert_eq!(min_max(&data), Some((0.5, 0.5)));
    assert!((median(&data) - 0.5).abs() < 1e-6);
}

#[test]
fn an_entirely_null_input_yields_zeros_not_nan() {
    let data = vec![0.0f32; 64];
    assert_eq!(mean_sigma(&data), (0, 0.0, 0.0));
    assert_eq!(min_max(&data), None);
    assert_eq!(median(&data), 0.0);
    assert_eq!(mad(&data, 0.0), 0.0);
    assert_eq!(background_noise(&data, 8, 8), 0.0);
}

// ---------------------------------------------------------------------------
// Median and MAD
// ---------------------------------------------------------------------------

#[test]
fn median_matches_a_full_sort() {
    let mut g = Gauss::new(99);
    for n in [1usize, 2, 3, 4, 5, 17, 64, 1000, 4095] {
        let data: Vec<f32> = (0..n).map(|_| g.next() as f32).collect();

        let mut sorted = data.clone();
        sorted.sort_by(f32::total_cmp);
        let expected = if n % 2 == 1 {
            sorted[n / 2]
        } else {
            (sorted[n / 2 - 1] + sorted[n / 2]) * 0.5
        };

        let mut scratch = data.clone();
        let got = median_in_place(&mut scratch);
        assert!((got - expected).abs() < 1e-6, "n={n}: {got} != {expected}");
    }
}

#[test]
fn median_is_linear_on_duplicate_heavy_data() {
    // The case real frames actually present: millions of pixels spread over a
    // few hundred distinct values. A two-way partition puts every duplicate of
    // the pivot on one side and advances by a single element per pass, which
    // is quadratic — at this size that is hours, not milliseconds.
    //
    // 4M samples over ~120 distinct values, matching a bias frame's spread.
    let n = 4_000_000usize;
    let mut g = Gauss::new(101);
    let data: Vec<f32> = (0..n)
        .map(|_| (302.0 + 6.0 * g.next()).round() as f32)
        .collect();

    let started = std::time::Instant::now();
    let mut scratch = data.clone();
    let got = median_in_place(&mut scratch);
    let elapsed = started.elapsed();

    let mut sorted = data.clone();
    sorted.sort_by(f32::total_cmp);
    let expected = (sorted[n / 2 - 1] + sorted[n / 2]) * 0.5;
    assert!((got - expected).abs() < 1e-6, "{got} != {expected}");

    // Generous enough not to be flaky under a loaded CI machine or a debug
    // build, tight enough that quadratic behaviour cannot pass.
    assert!(
        elapsed.as_secs() < 10,
        "median of {n} duplicate-heavy samples took {elapsed:?} — selection is not linear"
    );
}

/// The counting path and the quickselect path must produce the *same number*,
/// not merely a close one: the median sets the detection threshold, and a
/// threshold that moved when a frame crossed a size boundary would make star
/// lists depend on the frame's dimensions.
#[test]
fn parallel_and_serial_medians_agree_exactly() {
    // Comfortably over the size at which `median` switches to the counting
    // passes, so both paths are genuinely exercised.
    let n = 400_000usize;
    let mut g = Gauss::new(4242);

    let cases: Vec<(&str, Vec<f32>)> = vec![
        // Continuous values, odd and even counts.
        ("normal odd", (0..n + 1).map(|_| g.next() as f32).collect()),
        ("normal even", (0..n).map(|_| g.next() as f32).collect()),
        // A calibration frame: millions of samples over a few hundred levels,
        // where most samples share a bucket and the second pass has to resolve
        // them.
        (
            "quantized",
            (0..n)
                .map(|_| (302.0 + 6.0 * g.next()).round() as f32)
                .collect(),
        ),
        // Negative values exercise the other branch of the order key, and a
        // calibrated light frame genuinely contains them.
        (
            "signed",
            (0..n).map(|_| -0.5 + g.next() as f32 * 0.01).collect(),
        ),
        // Zero is null, so these samples must be skipped by both paths alike.
        (
            "half null",
            (0..n)
                .map(|i| if i % 2 == 0 { 0.0 } else { g.next() as f32 })
                .collect(),
        ),
        ("all identical", vec![0.125f32; n]),
    ];

    for (name, data) in cases {
        let mut kept = non_null(&data);
        let expected = median_in_place(&mut kept);
        let got = median(&data);
        assert_eq!(
            got.to_bits(),
            expected.to_bits(),
            "{name}: parallel median {got} != serial {expected}"
        );

        // The MAD shares the counting machinery and adds the case that
        // separates nullness from the mapped value: a sample sitting exactly
        // at the centre deviates by zero and is still data.
        let mut devs: Vec<f32> = data
            .iter()
            .copied()
            .filter(|v| *v != 0.0 && !v.is_nan())
            .map(|v| (v - expected).abs())
            .collect();
        let expected_mad = median_in_place(&mut devs);
        let got_mad = mad(&data, expected);
        assert_eq!(
            got_mad.to_bits(),
            expected_mad.to_bits(),
            "{name}: parallel mad {got_mad} != serial {expected_mad}"
        );
    }
}

#[test]
fn median_of_a_single_repeated_value_is_that_value() {
    // Degenerate version of the same hazard: every sample identical, so every
    // pivot equals every element.
    let mut data = vec![7.5f32; 1_000_000];
    assert_eq!(median_in_place(&mut data), 7.5);
}

#[test]
fn median_survives_already_sorted_input() {
    // Sorted and reverse-sorted runs are quickselect's classic quadratic
    // cases, and real image rows are full of them.
    let n = 20_001;
    let ascending: Vec<f32> = (0..n).map(|i| i as f32).collect();
    let mut scratch = ascending.clone();
    assert_eq!(median_in_place(&mut scratch), (n / 2) as f32);

    let mut descending: Vec<f32> = ascending.iter().rev().copied().collect();
    assert_eq!(median_in_place(&mut descending), (n / 2) as f32);

    let mut constant = vec![3.0f32; n];
    assert_eq!(median_in_place(&mut constant), 3.0);
}

#[test]
fn mad_recovers_the_sigma_of_a_normal_field() {
    // For normal data, MAD * 1.4826 estimates sigma.
    let data = noisy_field(512, 512, 0.3, 0.02, 11);
    let m = median(&data);
    let estimated = mad(&data, m) as f64 * stats::MAD_TO_SIGMA;
    assert!(
        (estimated - 0.02).abs() < 5e-4,
        "MAD-derived sigma {estimated}"
    );
}

#[test]
fn mad_is_unmoved_by_outliers_that_wreck_sigma() {
    // The whole point of a robust estimator: 2% hot pixels.
    let mut data = noisy_field(256, 256, 0.3, 0.02, 13);
    for (i, v) in data.iter_mut().enumerate() {
        if i % 50 == 0 {
            *v = 0.99;
        }
    }

    let m = median(&data);
    let robust = mad(&data, m) as f64 * stats::MAD_TO_SIGMA;
    let (_, _, classical) = mean_sigma(&data);

    assert!((robust - 0.02).abs() < 1e-3, "robust sigma {robust}");
    assert!(
        classical > 0.05,
        "classical sigma should be wrecked: {classical}"
    );
}

// ---------------------------------------------------------------------------
// Robust location and scale
// ---------------------------------------------------------------------------

#[test]
fn robust_scale_tracks_sigma_on_clean_data() {
    let data = noisy_field(512, 512, 0.4, 0.015, 17);
    let m = median(&data);
    let d = mad(&data, m);
    let (location, scale) = location_scale(&data, m, d).expect("estimable");

    assert!((location - 0.4).abs() < 1e-3, "location {location}");
    // The clip to +/-6 MAD and the consistency factor leave a few percent.
    assert!(
        (scale - 0.015).abs() / 0.015 < 0.05,
        "scale {scale} is not within 5% of 0.015"
    );
}

#[test]
fn robust_scale_ignores_a_bright_star_field() {
    let mut data = noisy_field(512, 512, 0.4, 0.015, 19);
    for (i, v) in data.iter_mut().enumerate() {
        if i % 200 == 0 {
            *v = 0.95;
        }
    }

    let m = median(&data);
    let d = mad(&data, m);
    let (location, scale) = location_scale(&data, m, d).expect("estimable");

    assert!((location - 0.4).abs() < 1e-3, "location {location}");
    assert!(
        (scale - 0.015).abs() / 0.015 < 0.05,
        "scale {scale} moved by more than 5%"
    );
}

#[test]
fn a_frame_with_no_spread_reports_no_estimate() {
    let flat = vec![0.5f32; 1024];
    assert_eq!(location_scale(&flat, 0.5, 0.0), None);
    assert_eq!(biweight_midvariance(&flat, 0.5, 0.0), 0.0);
}

#[test]
fn lite_estimates_approximate_the_robust_ones() {
    // The cheap pair is what long-sequence normalization uses; it is only
    // worth having if it stays close.
    let data = noisy_field(512, 512, 0.4, 0.015, 23);
    let m = median(&data);
    let d = mad(&data, m);

    let (_, scale) = location_scale(&data, m, d).expect("estimable");
    let (lite_location, lite_scale) = stats::lite_location_scale(m, d);

    assert!((lite_location - 0.4).abs() < 1e-3);
    assert!(
        (lite_scale - scale).abs() / scale < 0.1,
        "lite scale {lite_scale} vs robust {scale}"
    );
}

// ---------------------------------------------------------------------------
// Background noise
// ---------------------------------------------------------------------------

#[test]
fn background_noise_recovers_injected_sigma() {
    let (w, h) = (512usize, 512usize);
    for sigma in [0.001f32, 0.005, 0.02] {
        let data = noisy_field(w, h, 0.3, sigma, 31);
        let got = background_noise(&data, w, h);
        assert!(
            (got - sigma as f64).abs() / (sigma as f64) < 0.05,
            "sigma {sigma}: estimated {got}, off by more than 5%"
        );
    }
}

#[test]
fn background_noise_ignores_a_smooth_gradient() {
    // A sky gradient varies slowly across the frame, so differencing adjacent
    // pixels should barely see it. This is why the estimator differences
    // rather than taking the frame's spread.
    let (w, h) = (256usize, 256usize);
    let mut g = Gauss::new(37);
    let flat: Vec<f32> = (0..w * h).map(|_| 0.3 + 0.004 * g.next() as f32).collect();

    let mut g = Gauss::new(37);
    let gradient: Vec<f32> = (0..w * h)
        .map(|i| {
            let (x, y) = (i % w, i / w);
            let ramp = 0.3 * (x as f32 / w as f32) + 0.2 * (y as f32 / h as f32);
            0.3 + ramp + 0.004 * g.next() as f32
        })
        .collect();

    let flat_noise = background_noise(&flat, w, h);
    let gradient_noise = background_noise(&gradient, w, h);

    assert!(
        (flat_noise - 0.004).abs() / 0.004 < 0.05,
        "flat {flat_noise}"
    );
    assert!(
        (gradient_noise - flat_noise).abs() / flat_noise < 0.1,
        "gradient moved the estimate: {gradient_noise} vs {flat_noise}"
    );
}

#[test]
fn background_noise_ignores_stars() {
    let (w, h) = (256usize, 256usize);
    let mut data = noisy_field(w, h, 0.2, 0.003, 41);
    // Bright pixels scattered over ~1% of the frame.
    for (i, v) in data.iter_mut().enumerate() {
        if i % 97 == 0 {
            *v = 0.9;
        }
    }

    let got = background_noise(&data, w, h);
    assert!(
        (got - 0.003).abs() / 0.003 < 0.1,
        "stars moved the noise estimate: {got}"
    );
}

#[test]
fn rows_too_short_to_difference_yield_no_estimate() {
    assert_eq!(background_noise(&[1.0, 2.0], 2, 1), 0.0);
    assert_eq!(background_noise(&[], 0, 0), 0.0);
}

// ---------------------------------------------------------------------------
// Determinism
// ---------------------------------------------------------------------------

#[test]
fn statistics_do_not_depend_on_the_thread_count() {
    // Reductions are chunked at a fixed size rather than split per-thread, so
    // every one of these must agree bit for bit. If they ever stop agreeing,
    // every parity tolerance downstream becomes unreproducible.
    let (w, h) = (301usize, 197usize);
    let data = noisy_field(w, h, 0.35, 0.011, 43);

    let mut results = Vec::new();
    for cores in [1usize, 2, 3, 8] {
        let runtime = Runtime::new(ResourceBudget::new(1 << 30, cores)).unwrap();
        results.push(runtime.install(|| ChannelStats::compute(&data, w, h)));
    }

    let first = results[0];
    for (i, got) in results.iter().enumerate().skip(1) {
        assert_eq!(
            *got, first,
            "result {i} differs from the single-threaded one"
        );
    }
}

#[test]
fn channel_stats_agree_with_the_individual_estimators() {
    let (w, h) = (128usize, 96usize);
    let data = noisy_field(w, h, 0.22, 0.008, 47);
    let stats = ChannelStats::compute(&data, w, h);

    let (count, mean, sigma) = mean_sigma(&data);
    assert_eq!(stats.count, count);
    assert_eq!(stats.mean, mean);
    assert_eq!(stats.sigma, sigma);
    assert_eq!(stats.median, median(&data));
    assert_eq!(stats.background_noise, background_noise(&data, w, h));
    assert_eq!(min_max(&data), Some((stats.min, stats.max)));
    assert!(stats.scale > 0.0 && stats.location > 0.0);
}

#[test]
fn channel_stats_stay_finite_on_a_degenerate_frame() {
    // A perfectly flat frame has no spread; the robust estimators cannot
    // converge, and the fallbacks must still produce usable numbers.
    let (w, h) = (32usize, 32usize);
    let flat = vec![0.5f32; w * h];
    let stats = ChannelStats::compute(&flat, w, h);

    assert_eq!(stats.median, 0.5);
    assert_eq!(stats.mad, 0.0);
    assert_eq!(stats.scale, 0.0);
    assert_eq!(stats.location, 0.5);
    assert!(stats.sigma.is_finite() && stats.background_noise.is_finite());
}
