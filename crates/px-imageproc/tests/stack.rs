//! Stacking tests (ADR 007, Phase 3).
//!
//! Correctness here is checked against values computed by hand or by an
//! obviously-correct reference in the test itself, never against the
//! implementation's own output.

use std::path::{Path, PathBuf};

use px_fits::card::Value;
use px_fits::{BitPix, FitsWriter, HeaderBuilder};
use px_imageproc::runtime::{ImageDims, ResourceBudget, Runtime};
use px_imageproc::stack::{Bounds, Method, Rejection, StackOptions, stack};
use px_imageproc::{Frame, FrameSet};

const U16: f32 = 65535.0;
const MIB: u64 = 1 << 20;
const GB: u64 = 1 << 30;

fn dir(name: &str) -> PathBuf {
    let d = std::env::temp_dir()
        .join("px-imageproc-stack-tests")
        .join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).expect("create test dir");
    d
}

/// Writes a set of frames from per-frame pixel values given in ADU, so tests
/// can state expectations in the units they were written in.
fn write_set(d: &Path, frames: &[Vec<u16>], width: usize, height: usize) -> Vec<PathBuf> {
    frames
        .iter()
        .enumerate()
        .map(|(i, data)| {
            assert_eq!(data.len(), width * height);
            let path = d.join(format!("f{i:03}.fits"));
            let header = HeaderBuilder::primary_image(BitPix::I16, &[width as u64, height as u64])
                .expect("header")
                .card("BZERO", Value::Integer(32768), None)
                .expect("bzero");
            // BZERO 32768 means the stored value is the ADU minus 32768.
            let stored: Vec<i16> = data.iter().map(|&v| (v as i32 - 32768) as i16).collect();
            let mut w = FitsWriter::create(&path).expect("create");
            w.write_image(&header, &stored).expect("write");
            w.finish().expect("finish");
            path
        })
        .collect()
}

/// One-pixel frames, one value each — the clearest way to test a reducer.
fn single_pixel_set(d: &Path, values: &[u16]) -> FrameSet {
    let frames: Vec<Vec<u16>> = values.iter().map(|&v| vec![v]).collect();
    let paths = write_set(d, &frames, 1, 1);
    FrameSet::open(&paths).expect("open set")
}

fn runtime(cores: usize) -> Runtime {
    Runtime::new(ResourceBudget::new(GB, cores)).expect("runtime")
}

/// The stacked value of the single pixel, in ADU.
fn stack_one(set: &FrameSet, method: Method) -> f32 {
    let result = stack(set, &runtime(1), StackOptions::new(method)).expect("stack");
    result.frame.data()[0] * U16
}

// ---------------------------------------------------------------------------
// Reducers
// ---------------------------------------------------------------------------

#[test]
fn median_of_an_odd_stack_is_the_middle_value() {
    let d = dir("median-odd");
    let set = single_pixel_set(&d, &[100, 500, 200, 400, 300]);
    assert!((stack_one(&set, Method::Median) - 300.0).abs() < 0.01);
}

#[test]
fn median_of_an_even_stack_averages_the_two_middle_values() {
    let d = dir("median-even");
    let set = single_pixel_set(&d, &[100, 200, 300, 400]);
    assert!((stack_one(&set, Method::Median) - 250.0).abs() < 0.01);
}

#[test]
fn median_ignores_a_single_wild_outlier() {
    // The property that makes median stacking right for calibration masters.
    let d = dir("median-outlier");
    let set = single_pixel_set(&d, &[300, 302, 301, 303, 65535]);
    assert!((stack_one(&set, Method::Median) - 302.0).abs() < 0.01);
}

#[test]
fn min_max_and_sum_do_what_they_say() {
    let d = dir("minmaxsum");
    let set = single_pixel_set(&d, &[100, 500, 200]);
    assert!((stack_one(&set, Method::Min) - 100.0).abs() < 0.01);
    assert!((stack_one(&set, Method::Max) - 500.0).abs() < 0.01);
    assert!((stack_one(&set, Method::Sum) - 800.0).abs() < 0.02);
}

#[test]
fn mean_without_rejection_averages_everything() {
    let d = dir("mean-plain");
    let set = single_pixel_set(&d, &[100, 200, 300, 400]);
    let got = stack_one(&set, Method::Mean(Rejection::None));
    assert!((got - 250.0).abs() < 0.01, "{got}");
}

#[test]
fn median_counts_zeros_but_mean_does_not() {
    // A zero means "no data here" to the mean, which drops it, and "a real
    // reading" to the median, which keeps it. The two therefore disagree, and
    // that disagreement is deliberate.
    let d = dir("zeros");
    let set = single_pixel_set(&d, &[0, 0, 300, 300, 300]);

    // Mean over the three non-null values.
    let mean = stack_one(&set, Method::Mean(Rejection::None));
    assert!((mean - 300.0).abs() < 0.01, "mean {mean}");

    // Median over all five, so the two zeros drag it down.
    let median = stack_one(&set, Method::Median);
    assert!((median - 300.0).abs() < 0.01, "median {median}");

    let set = single_pixel_set(&dir("zeros2"), &[0, 0, 0, 300, 300]);
    let median = stack_one(&set, Method::Median);
    assert!(median.abs() < 0.01, "median of mostly-zero stack: {median}");
    let mean = stack_one(&set, Method::Mean(Rejection::None));
    assert!((mean - 300.0).abs() < 0.01, "mean {mean}");
}

// ---------------------------------------------------------------------------
// Rejection
// ---------------------------------------------------------------------------

/// A tight cluster plus outliers — the situation every rejection method
/// exists to handle.
fn clustered(outliers: &[u16]) -> Vec<u16> {
    let mut v: Vec<u16> = (0..20).map(|i| 1000 + (i % 5) as u16).collect();
    v.extend_from_slice(outliers);
    v
}

#[test]
fn every_rejection_method_removes_a_bright_outlier() {
    let clean: Vec<u16> = (0..20).map(|i| 1000 + (i % 5) as u16).collect();
    let expected = clean.iter().map(|&v| v as f64).sum::<f64>() / clean.len() as f64;

    for (name, rule) in [
        ("sigma", Rejection::Sigma(Bounds::symmetric(3.0))),
        ("mad", Rejection::Mad(Bounds::symmetric(3.0))),
        ("winsorized", Rejection::Winsorized(Bounds::symmetric(3.0))),
        ("linearfit", Rejection::LinearFit(Bounds::symmetric(5.0))),
        (
            "sigmamedian",
            Rejection::SigmaMedian(Bounds::symmetric(3.0)),
        ),
    ] {
        let d = dir(&format!("reject-{name}"));
        let set = single_pixel_set(&d, &clustered(&[60000]));
        let got = stack_one(&set, Method::Mean(rule));

        assert!(
            (got - expected as f32).abs() < 2.0,
            "{name}: got {got}, expected about {expected} — the outlier was not removed"
        );
    }
}

#[test]
fn rejection_counts_are_reported_and_attributed_to_the_right_tail() {
    let d = dir("reject-counts");
    let set = single_pixel_set(&d, &clustered(&[60000, 60001]));
    let result = stack(
        &set,
        &runtime(1),
        StackOptions::new(Method::Mean(Rejection::Sigma(Bounds::symmetric(3.0)))),
    )
    .expect("stack");

    assert_eq!(
        result.rejected.high, 2,
        "both outliers are on the high side"
    );
    assert_eq!(result.rejected.low, 0);
}

#[test]
fn rejection_maps_record_per_pixel_counts() {
    // Two pixels: the first clean, the second with an outlier in one frame.
    let d = dir("reject-maps");
    let frames: Vec<Vec<u16>> = (0..10)
        .map(|i| vec![1000, if i == 3 { 60000 } else { 1000 }])
        .collect();
    let paths = write_set(&d, &frames, 2, 1);
    let set = FrameSet::open(&paths).expect("open");

    let result = stack(
        &set,
        &runtime(1),
        StackOptions::new(Method::Mean(Rejection::Winsorized(Bounds::symmetric(3.0))))
            .rejection_maps(true),
    )
    .expect("stack");

    let high = result.rejection_map_high.expect("high map requested");
    let low = result.rejection_map_low.expect("low map requested");
    assert_eq!(high[0], 0, "clean pixel had a rejection");
    assert_eq!(high[1], 1, "outlier pixel should have one high rejection");
    assert_eq!(low, vec![0, 0]);
}

#[test]
fn percentile_rejection_works_where_a_spread_estimate_would_not() {
    // Four frames: too few for a trustworthy sigma, which is exactly when
    // percentile clipping is selected.
    let d = dir("percentile");
    let set = single_pixel_set(&d, &[1000, 1010, 990, 60000]);
    let got = stack_one(
        &set,
        Method::Mean(Rejection::Percentile(Bounds::new(0.2, 0.1))),
    );
    assert!((got - 1000.0).abs() < 5.0, "got {got}");
}

#[test]
fn sigma_median_substitutes_rather_than_dropping() {
    // The distinguishing property: the outlier is replaced by the median, so
    // the average is still taken over every frame. With 20 clean values near
    // 1002 and one at 60000, the result should be the mean of 21 values where
    // the 21st has become the median — not the mean of the 20 survivors.
    let d = dir("sigmamedian");
    let values = clustered(&[60000]);
    let set = single_pixel_set(&d, &values);
    let got = stack_one(
        &set,
        Method::Mean(Rejection::SigmaMedian(Bounds::symmetric(3.0))),
    );

    let clean: Vec<f64> = values[..20].iter().map(|&v| v as f64).collect();
    let mut sorted = clean.clone();
    sorted.sort_by(f64::total_cmp);
    let median = (sorted[9] + sorted[10]) * 0.5;
    let expected = (clean.iter().sum::<f64>() + median) / 21.0;

    assert!(
        (got as f64 - expected).abs() < 1.0,
        "got {got}, expected {expected}"
    );
}

#[test]
fn sigma_clipping_cannot_catch_an_outlier_that_dominates_a_tiny_stack() {
    // Worth pinning because it is counter-intuitive and it is why
    // `BestRejection` selects percentile clipping below seven frames: with
    // five values, one extreme outlier inflates the standard deviation so
    // much that it falls inside its own 3-sigma bound and survives.
    let d = dir("small-stack-sigma");
    let set = single_pixel_set(&d, &[1000, 1000, 1000, 1000, 60000]);

    let sigma = stack_one(&set, Method::Mean(Rejection::Sigma(Bounds::symmetric(3.0))));
    assert!((sigma - 12800.0).abs() < 1.0, "sigma clipping: {sigma}");

    // Percentile clipping, which does not estimate a spread, handles it.
    let percentile = stack_one(
        &set,
        Method::Mean(Rejection::Percentile(Bounds::new(0.2, 0.1))),
    );
    assert!(
        (percentile - 1000.0).abs() < 1.0,
        "percentile: {percentile}"
    );
}

#[test]
fn rejection_stops_before_the_stack_is_gutted() {
    // Values with no cluster at all: a naive loop would keep clipping until
    // almost nothing was left.
    let d = dir("floor");
    let set = single_pixel_set(&d, &[100, 2000, 8000, 20000, 45000, 60000]);
    let result = stack(
        &set,
        &runtime(1),
        StackOptions::new(Method::Mean(Rejection::Sigma(Bounds::symmetric(1.0)))),
    )
    .expect("stack");

    let kept = 6 - result.rejected.total();
    assert!(kept >= 4, "rejection left only {kept} of 6 values");
    assert!(result.frame.data()[0] > 0.0, "result collapsed to zero");
}

#[test]
fn gesdt_is_a_typed_error_not_a_silent_fallback() {
    let d = dir("gesdt");
    let set = single_pixel_set(&d, &[100, 200, 300]);
    let err = stack(
        &set,
        &runtime(1),
        StackOptions::new(Method::Mean(Rejection::Gesdt(Bounds::new(0.3, 0.05)))),
    )
    .unwrap_err();
    assert!(err.to_string().contains("GESDT"), "{err}");
}

// ---------------------------------------------------------------------------
// Block scheduling
// ---------------------------------------------------------------------------

/// Stacks a multi-row image and checks every pixel, so a block boundary that
/// scrambled rows would show up.
fn gradient_frames(width: usize, height: usize, count: usize, channels: usize) -> Vec<Vec<u16>> {
    (0..count)
        .map(|f| {
            (0..width * height * channels)
                .map(|i| (100 + (i * 3 + f) % 60_000) as u16)
                .collect()
        })
        .collect()
}

#[test]
fn a_block_split_result_matches_a_single_block_result() {
    // The invariant the whole memory plan rests on: how the image is carved
    // up must not change a single output pixel.
    //
    // The image has to be large enough that a budget above the 1 MiB
    // quantization floor still forces many blocks.
    let (w, h) = (512usize, 512usize);
    let frames = gradient_frames(w, h, 20, 1);
    let d = dir("blocks");
    let paths = write_set(&d, &frames, w, h);
    let set = FrameSet::open(&paths).expect("open");

    let roomy = stack(
        &set,
        &Runtime::new(ResourceBudget::new(GB, 1)).unwrap(),
        StackOptions::new(Method::Median),
    )
    .expect("stack");

    let tight_budget = ResourceBudget::new(4 * MIB, 4);
    let tight = stack(
        &set,
        &Runtime::new(tight_budget).unwrap(),
        StackOptions::new(Method::Median),
    )
    .expect("stack");

    assert!(
        tight.block_count > roomy.block_count,
        "tight budget produced {} blocks, roomy produced {}",
        tight.block_count,
        roomy.block_count
    );
    assert_eq!(
        roomy.frame.data(),
        tight.frame.data(),
        "block layout changed the result"
    );
}

#[test]
fn results_do_not_depend_on_the_core_count() {
    let (w, h) = (256usize, 256usize);
    let frames = gradient_frames(w, h, 9, 1);
    let d = dir("cores");
    let paths = write_set(&d, &frames, w, h);
    let set = FrameSet::open(&paths).expect("open");

    let options = StackOptions::new(Method::Mean(Rejection::Winsorized(Bounds::symmetric(3.0))));
    let mut results = Vec::new();
    for cores in [1usize, 2, 3, 8] {
        let budget = ResourceBudget::new(2 * MIB, cores);
        let r = stack(&set, &Runtime::new(budget).unwrap(), options).expect("stack");
        results.push(r.frame.into_data());
    }

    for (i, got) in results.iter().enumerate().skip(1) {
        assert_eq!(*got, results[0], "core count {i} changed the result");
    }
}

#[test]
fn rgb_frames_stack_channel_by_channel() {
    let (w, h) = (64usize, 64usize);
    let frames = gradient_frames(w, h, 5, 3);
    let d = dir("rgb");

    let paths: Vec<PathBuf> = frames
        .iter()
        .enumerate()
        .map(|(i, data)| {
            let path = d.join(format!("f{i:03}.fits"));
            let header = HeaderBuilder::primary_image(BitPix::I16, &[w as u64, h as u64, 3])
                .expect("header");
            let stored: Vec<i16> = data.iter().map(|&v| v as i16).collect();
            let mut wr = FitsWriter::create(&path).expect("create");
            wr.write_image(&header, &stored).expect("write");
            wr.finish().expect("finish");
            path
        })
        .collect();

    let set = FrameSet::open(&paths).expect("open");
    assert_eq!(set.dims(), ImageDims::new(w, h, 3));

    let result = stack(
        &set,
        &Runtime::new(ResourceBudget::new(2 * MIB, 4)).unwrap(),
        StackOptions::new(Method::Median),
    )
    .expect("stack");

    assert_eq!(result.frame.channels(), 3);
    // Each channel's gradient continues from the previous one, so the
    // channels must differ — a bug that collapsed them would show here.
    let c0 = result.frame.channel(0)[0];
    let c1 = result.frame.channel(1)[0];
    let c2 = result.frame.channel(2)[0];
    assert!(c0 < c1 && c1 < c2, "{c0} {c1} {c2}");
}

// ---------------------------------------------------------------------------
// Output range
// ---------------------------------------------------------------------------

#[test]
fn without_output_norm_the_result_is_clamped_to_the_unit_range() {
    let d = dir("clamp");
    // Sum of four bright frames overflows the unit range.
    let set = single_pixel_set(&d, &[40000, 40000, 40000, 40000]);
    let result = stack(&set, &runtime(1), StackOptions::new(Method::Sum)).expect("stack");
    assert_eq!(result.frame.data()[0], 1.0);
}

#[test]
fn output_norm_rescales_the_span_and_leaves_gaps_at_zero() {
    let (w, h) = (4usize, 1usize);
    let d = dir("outnorm");
    // One pixel absent in every frame, the rest spanning a narrow band.
    let frames: Vec<Vec<u16>> = (0..3).map(|_| vec![0, 1000, 1500, 2000]).collect();
    let paths = write_set(&d, &frames, w, h);
    let set = FrameSet::open(&paths).expect("open");

    let result = stack(
        &set,
        &runtime(1),
        StackOptions::new(Method::Median).output_norm(true),
    )
    .expect("stack");

    let data = result.frame.data();
    assert_eq!(data[0], 0.0, "an absent pixel must stay absent");
    assert!((data[1] - 0.0).abs() < 1e-6, "min maps to 0: {data:?}");
    assert!((data[3] - 1.0).abs() < 1e-6, "max maps to 1: {data:?}");
    assert!((data[2] - 0.5).abs() < 1e-6, "midpoint: {data:?}");
}

// ---------------------------------------------------------------------------
// Errors
// ---------------------------------------------------------------------------

#[test]
fn stacking_nothing_is_an_error() {
    let d = dir("empty");
    let paths = write_set(&d, &[vec![100u16]], 1, 1);
    let set = FrameSet::open(&paths).expect("open");
    // A one-frame set is legal; an empty one never gets that far.
    assert!(stack(&set, &runtime(1), StackOptions::new(Method::Median)).is_ok());
    assert!(FrameSet::open(&[]).is_err());
}

#[test]
fn a_budget_too_small_for_the_result_is_reported() {
    let (w, h) = (256usize, 256usize);
    let frames = gradient_frames(w, h, 4, 1);
    let d = dir("tiny-budget");
    let paths = write_set(&d, &frames, w, h);
    let set = FrameSet::open(&paths).expect("open");

    // A budget under the 1 MiB quantization floor can never fit anything.
    let budget = ResourceBudget::new(64 * 1024, 2);
    let err = stack(
        &set,
        &Runtime::new(budget).unwrap(),
        StackOptions::new(Method::Median),
    )
    .unwrap_err();
    assert!(
        err.to_string().contains("memory budget too small"),
        "expected a budget error, got: {err}"
    );
}

#[test]
fn a_frame_can_be_written_and_read_back_after_stacking() {
    let d = dir("write");
    let set = single_pixel_set(&d, &[100, 200, 300]);
    let result = stack(&set, &runtime(1), StackOptions::new(Method::Median)).expect("stack");

    let out = d.join("master.fits");
    result
        .frame
        .write(&out, &[("STACKCNT", Value::Integer(3), None)])
        .expect("write");

    let reader = px_fits::reader::FitsReader::open(&out).expect("open");
    let hdu = reader.primary_image().expect("image");
    let back = Frame::from_hdu(&hdu).expect("decode");
    assert!((back.data()[0] * U16 - 200.0).abs() < 0.01);
    assert_eq!(back.header().get_i64("STACKCNT"), Some(3));
}
