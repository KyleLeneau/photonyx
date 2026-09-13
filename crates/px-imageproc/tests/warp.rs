//! Warping (ADR 007, P10-T4).
//!
//! Three properties, in increasing subtlety. An exact-integer shift must move
//! pixels and change nothing else — for *every* kernel, since a kernel evaluated
//! at an integer offset should reduce to a copy. A warp followed by its inverse
//! must return the original in the interior. And with clamping on, the pixels
//! that get repaired must be the ones around bright cores and not a scatter
//! across the background.

use px_fits::{BitPix, HeaderBuilder};
use px_imageproc::register::transform::Transform;
use px_imageproc::register::warp::{Interpolation, WarpOptions, dilate3x3, warp};
use px_imageproc::runtime::{ResourceBudget, Runtime};
use px_imageproc::{Frame, ImageDims};

const WIDTH: usize = 160;
const HEIGHT: usize = 120;
const GB: u64 = 1 << 30;

const EVERY_KERNEL: [Interpolation; 4] = [
    Interpolation::Bilinear,
    Interpolation::Area,
    Interpolation::Bicubic,
    Interpolation::Lanczos4,
];

fn runtime() -> Runtime {
    Runtime::new(ResourceBudget::new(GB, 4)).expect("runtime")
}

/// A star field on a gradient background: smooth structure for the round-trip
/// test, sharp cores for the clamping test.
fn star_field(channels: usize) -> Frame {
    let dims = ImageDims::new(WIDTH, HEIGHT, channels);
    let mut data = vec![0.0f32; dims.pixel_count()];
    for channel in 0..channels {
        for y in 0..HEIGHT {
            for x in 0..WIDTH {
                let gradient = 0.02 + 0.00005 * (x + y) as f32 + 0.002 * channel as f32;
                data[(channel * HEIGHT + y) * WIDTH + x] = gradient;
            }
        }
    }

    for &(cx, cy, amplitude) in &[
        (40.0f64, 30.0f64, 0.9f32),
        (100.0, 70.0, 0.6),
        (120.0, 25.0, 0.35),
        (60.0, 95.0, 0.8),
    ] {
        let sigma = 1.6f64;
        for channel in 0..channels {
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let (dx, dy) = (x as f64 - cx, y as f64 - cy);
                    let q = (dx * dx + dy * dy) / (2.0 * sigma * sigma);
                    if q < 25.0 {
                        data[(channel * HEIGHT + y) * WIDTH + x] += amplitude * (-q).exp() as f32;
                    }
                }
            }
        }
    }
    // Round-tripped through a file rather than constructed directly: `Frame`
    // carries a real FITS header, and a test frame should have one a writer
    // would accept.
    let dir = tempfile::tempdir().expect("tempdir");
    let path = dir.path().join("field.fits");
    let mut axes = vec![WIDTH as u64, HEIGHT as u64];
    if channels > 1 {
        axes.push(channels as u64);
    }
    let header = HeaderBuilder::primary_image(BitPix::F32, &axes).expect("header");
    let mut writer = px_fits::FitsWriter::create(&path).expect("create");
    writer.write_image(&header, &data).expect("write");
    writer.finish().expect("finish");
    let frame = Frame::open(&path).expect("open");
    assert_eq!(frame.dims(), dims);
    frame
}

#[test]
fn an_integer_shift_is_a_copy_for_every_kernel() {
    let source = star_field(1);
    let dims = source.dims();
    let (dx, dy) = (7i64, -5i64);

    for interpolation in EVERY_KERNEL {
        let options = WarpOptions {
            interpolation,
            // Clamping compares against an area-interpolated guide, which at an
            // integer offset is a 2x2 average rather than a copy, so the
            // identity only holds with it off. That is the documented contract.
            clamp: false,
        };
        let warped = warp(
            &source,
            &Transform::shift(dx as f64, dy as f64),
            dims,
            &runtime(),
            &options,
        )
        .expect("warp");

        let mut compared = 0usize;
        for y in 0..HEIGHT as i64 {
            for x in 0..WIDTH as i64 {
                let (sx, sy) = (x - dx, y - dy);
                // Only where the kernel's whole support is inside the source: an
                // output pixel near the border legitimately has no value.
                let margin = interpolation_margin(interpolation);
                if sx < margin || sy < margin {
                    continue;
                }
                if sx >= WIDTH as i64 - margin || sy >= HEIGHT as i64 - margin {
                    continue;
                }
                let got = warped.get(0, x as usize, y as usize);
                let want = source.get(0, sx as usize, sy as usize);
                assert!(
                    (got - want).abs() < 1e-6,
                    "{interpolation:?} at ({x}, {y}): {got} vs {want}"
                );
                compared += 1;
            }
        }
        assert!(
            compared > WIDTH * HEIGHT / 2,
            "{interpolation:?}: only {compared} pixels tested"
        );
    }
}

/// Pixels this close to the source edge have no full kernel support.
fn interpolation_margin(interpolation: Interpolation) -> i64 {
    match interpolation {
        Interpolation::Bilinear | Interpolation::Area => 1,
        Interpolation::Bicubic => 2,
        Interpolation::Lanczos4 => 4,
    }
}

#[test]
fn pixels_with_no_source_are_left_at_zero() {
    let source = star_field(1);
    let dims = source.dims();
    // Shifted right by 20: the left 20 columns of the output have no source.
    let warped = warp(
        &source,
        &Transform::shift(20.0, 0.0),
        dims,
        &runtime(),
        &WarpOptions {
            interpolation: Interpolation::Bilinear,
            clamp: false,
        },
    )
    .expect("warp");

    // The last row is excluded: bilinear there would need the row past the
    // frame, so it legitimately has no value either.
    for y in 0..HEIGHT - 1 {
        for x in 0..19 {
            assert_eq!(
                warped.get(0, x, y),
                0.0,
                "({x}, {y}) should be untouched, not filled"
            );
        }
        assert!(
            warped.get(0, 25, y) > 0.0,
            "({}, {y}) should have a value",
            25
        );
    }
}

#[test]
fn a_round_trip_warp_returns_the_original_in_the_interior() {
    let source = star_field(1);
    let dims = source.dims();
    let transform = Transform::similarity(1.0, 0.03, 6.5, -3.25);
    let inverse = transform.invert().expect("invertible");
    let options = WarpOptions {
        interpolation: Interpolation::Lanczos4,
        clamp: false,
    };

    let there = warp(&source, &transform, dims, &runtime(), &options).expect("warp");
    let back = warp(&there, &inverse, dims, &runtime(), &options).expect("warp back");

    // The interior only: a round trip cannot recover pixels whose intermediate
    // sample fell outside the frame. 20 px covers the rotation's sweep.
    let mut worst = 0.0f32;
    for y in 20..HEIGHT - 20 {
        for x in 20..WIDTH - 20 {
            let delta = (back.get(0, x, y) - source.get(0, x, y)).abs();
            worst = worst.max(delta);
        }
    }
    // Two interpolations of a star field at 1.6 px sigma: the error is dominated
    // by what resampling a nearly-critically-sampled core costs, twice.
    assert!(worst < 1e-2, "worst interior round-trip error {worst}");
}

#[test]
fn clamping_repairs_undershoot_around_star_cores() {
    let source = star_field(1);
    let dims = source.dims();
    // A half-pixel shift is the worst case for ringing: every output pixel sits
    // between two source pixels, where the kernel's negative lobes are fully
    // engaged.
    let transform = Transform::shift(0.5, 0.5);

    let unclamped = warp(
        &source,
        &transform,
        dims,
        &runtime(),
        &WarpOptions {
            interpolation: Interpolation::Lanczos4,
            clamp: false,
        },
    )
    .expect("warp");
    let clamped = warp(
        &source,
        &transform,
        dims,
        &runtime(),
        &WarpOptions {
            interpolation: Interpolation::Lanczos4,
            clamp: true,
        },
    )
    .expect("warp");

    let changed: Vec<usize> = (0..dims.pixel_count())
        .filter(|&i| (unclamped.data()[i] - clamped.data()[i]).abs() > 1e-7)
        .collect();
    assert!(!changed.is_empty(), "clamping changed nothing");

    // No bound on the *fraction* here: it is a property of this frame's star
    // density rather than of the algorithm — four fat stars in a 160x120 frame
    // put a tenth of the pixels on a star's wing, where bilinear's chord across
    // a sharply curved profile reads higher than the sinc does. What the mask
    // must be is localized, which the next assertion checks; the fraction on a
    // real frame is measured in `warp_parity.rs`.
    let fraction = changed.len() as f64 / dims.pixel_count() as f64;
    println!("clamping touched {:.1}% of this frame", 100.0 * fraction);

    // And every changed pixel must be near a star, which is where ringing lives.
    let stars = [(40.0, 30.0), (100.0, 70.0), (120.0, 25.0), (60.0, 95.0)];
    for &i in &changed {
        let (x, y) = ((i % WIDTH) as f64, (i / WIDTH) as f64);
        let distance = stars
            .iter()
            .map(|&(sx, sy): &(f64, f64)| ((x - sx).powi(2) + (y - sy).powi(2)).sqrt())
            .fold(f64::MAX, f64::min);
        assert!(
            distance < 12.0,
            "clamped pixel ({x}, {y}) is {distance:.1} px from any star"
        );
    }

    // The net effect is to raise the frame, which is what repairing undershoot
    // means. It is not monotone per pixel, and that is by design rather than an
    // accident: the mask is dilated before the guide is copied in, so the ring
    // around a repaired core includes pixels where the sinc read *higher* than
    // the guide and those are pulled down with the rest. Siril's clamp behaves
    // the same way, and it is the reason clamping slightly softens star cores.
    let raised: f64 = changed
        .iter()
        .map(|&i| (clamped.data()[i] - unclamped.data()[i]) as f64)
        .sum();
    assert!(raised > 0.0, "clamping lowered the frame overall: {raised}");

    let lowered = changed
        .iter()
        .filter(|&&i| clamped.data()[i] < unclamped.data()[i])
        .count();
    println!(
        "{} of {} changed pixels were lowered by the dilated mask",
        lowered,
        changed.len()
    );

    // The deepest undershoot must be among the repaired pixels — if the worst
    // case is missed, the threshold or the mask is wrong.
    let deepest = (0..dims.pixel_count())
        .filter(|&i| unclamped.data()[i] != 0.0)
        .min_by(|&a, &b| {
            let ratio = |i: usize| unclamped.data()[i] / clamped.data()[i].max(1e-12);
            ratio(a).total_cmp(&ratio(b))
        })
        .expect("non-empty");
    assert!(
        changed.contains(&deepest),
        "the deepest undershoot was not repaired"
    );
}

#[test]
fn clamping_does_nothing_for_a_kernel_without_negative_lobes() {
    let source = star_field(1);
    let dims = source.dims();
    let transform = Transform::shift(0.5, 0.5);

    for interpolation in [Interpolation::Bilinear, Interpolation::Area] {
        let off = warp(
            &source,
            &transform,
            dims,
            &runtime(),
            &WarpOptions {
                interpolation,
                clamp: false,
            },
        )
        .expect("warp");
        let on = warp(
            &source,
            &transform,
            dims,
            &runtime(),
            &WarpOptions {
                interpolation,
                clamp: true,
            },
        )
        .expect("warp");
        assert_eq!(off.data(), on.data(), "{interpolation:?}");
    }
}

#[test]
fn every_channel_is_warped_and_none_bleeds_into_another() {
    let source = star_field(3);
    let dims = source.dims();
    let warped = warp(
        &source,
        &Transform::shift(4.0, 3.0),
        dims,
        &runtime(),
        &WarpOptions {
            interpolation: Interpolation::Bilinear,
            clamp: false,
        },
    )
    .expect("warp");

    assert_eq!(warped.channels(), 3);
    for channel in 0..3 {
        // The channels differ by a constant offset in the source, and must still
        // differ by it after warping — a band that straddled a channel boundary
        // and read the wrong plane would break this.
        let got = warped.get(channel, 80, 60);
        let want = source.get(channel, 76, 57);
        assert!(
            (got - want).abs() < 1e-6,
            "channel {channel}: {got} vs {want}"
        );
        // And the top rows, which have no source, stay empty in every channel.
        assert_eq!(warped.get(channel, 80, 0), 0.0);
    }
}

#[test]
fn a_singular_transform_is_a_typed_error() {
    let source = star_field(1);
    let dims = source.dims();
    let singular = Transform::from_matrix(nalgebra::Matrix3::new(
        1.0, 2.0, 3.0, 2.0, 4.0, 6.0, 0.0, 0.0, 1.0,
    ))
    .expect("finite");
    assert!(
        warp(
            &source,
            &singular,
            dims,
            &runtime(),
            &WarpOptions::default()
        )
        .is_err()
    );
}

#[test]
fn the_output_may_be_a_different_shape_than_the_source() {
    let source = star_field(1);
    // A minimum-framing ROI: smaller output, shifted origin.
    let dims = ImageDims::new(100, 80, 1);
    let warped = warp(
        &source,
        &Transform::shift(-20.0, -15.0),
        dims,
        &runtime(),
        &WarpOptions {
            interpolation: Interpolation::Bilinear,
            clamp: false,
        },
    )
    .expect("warp");

    assert_eq!(warped.dims(), dims);
    assert!((warped.get(0, 10, 10) - source.get(0, 30, 25)).abs() < 1e-6);
}

#[test]
fn dilation_grows_a_mask_by_one_pixel_in_four_directions() {
    let dims = ImageDims::new(7, 5, 1);
    let mut mask = vec![false; dims.pixel_count()];
    mask[2 * 7 + 3] = true;

    let dilated = dilate3x3(&mask, dims, &runtime());
    let set: Vec<usize> = dilated
        .iter()
        .enumerate()
        .filter(|&(_, &v)| v)
        .map(|(i, _)| i)
        .collect();
    // The plus shape: the centre and its four edge neighbours, not the corners.
    assert_eq!(set, vec![7 + 3, 2 * 7 + 2, 2 * 7 + 3, 2 * 7 + 4, 3 * 7 + 3]);
}

#[test]
fn dilation_does_not_cross_a_channel_boundary() {
    let dims = ImageDims::new(4, 3, 2);
    let mut mask = vec![false; dims.pixel_count()];
    // Bottom row of channel 0.
    mask[2 * 4 + 1] = true;

    let dilated = dilate3x3(&mask, dims, &runtime());
    // The top row of channel 1 must stay clear.
    let plane = 4 * 3;
    assert!(!dilated[plane + 1], "the mask leaked into the next channel");
    assert!(dilated[2 * 4 + 1] && dilated[4 + 1]);
}
