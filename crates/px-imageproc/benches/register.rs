//! Star detection and PSF fitting throughput (ADR 007, P7-T7).
//!
//! The reference workload is a frame carrying 2000 fittable stars, which is the
//! default star cap and roughly what a rich field yields after rejection. Three
//! things are measured separately, because they scale differently: detection is
//! linear in pixels, fitting is linear in candidates, and the cap decides how
//! many candidates are fitted at all.

use criterion::{Criterion, criterion_group, criterion_main};
use px_imageproc::register::psf::PsfStar;
use px_imageproc::register::starfind::{detect_candidates, find_stars, find_stars_reported};
use px_imageproc::register::starmatch::{MatchOptions, match_stars, register_pair};
use px_imageproc::register::transform::{RansacOptions, Transform, TransformKind};
use px_imageproc::register::warp::{Interpolation, WarpOptions, warp};
use px_imageproc::runtime::{ResourceBudget, Runtime};
use px_imageproc::{Profile, StarfindOptions};

const GB: u64 = 1 << 30;
const WIDTH: usize = 2048;
const HEIGHT: usize = 2048;
const STARS: usize = 2000;

struct SplitMix64(u64);

impl SplitMix64 {
    /// Uniform in [0, 1).
    fn next_unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A star field: `STARS` Gaussians over a noisy background, spread on a jittered
/// grid so no two land in the same detection box.
fn star_field() -> Vec<f32> {
    let mut rng = SplitMix64(0x5741_1F7A_DEAD_BEEF);
    let mut data: Vec<f32> = (0..WIDTH * HEIGHT)
        .map(|_| (0.02 + 0.0008 * (rng.next_unit() - 0.5)) as f32)
        .collect();

    let per_side = (STARS as f64).sqrt().ceil() as usize;
    let spacing = (WIDTH - 40) as f64 / per_side as f64;
    for i in 0..STARS {
        let (column, row) = (i % per_side, i / per_side);
        let x0 = 20.0 + spacing * (column as f64 + 0.5) + 0.6 * (rng.next_unit() - 0.5);
        let y0 = 20.0 + spacing * (row as f64 + 0.5) + 0.6 * (rng.next_unit() - 0.5);
        let amplitude = 0.05 + 0.5 * rng.next_unit();
        let sigma = 1.5 + 0.8 * rng.next_unit();

        let reach = (6.0 * sigma).ceil() as isize;
        let (cx, cy) = (x0.round() as isize, y0.round() as isize);
        for y in (cy - reach).max(0)..(cy + reach + 1).min(HEIGHT as isize) {
            for x in (cx - reach).max(0)..(cx + reach + 1).min(WIDTH as isize) {
                let (dx, dy) = (x as f64 - x0, y as f64 - y0);
                let q = (dx * dx + dy * dy) / (2.0 * sigma * sigma);
                if q < 30.0 {
                    data[y as usize * WIDTH + x as usize] += (amplitude * (-q).exp()) as f32;
                }
            }
        }
    }
    data
}

fn uncapped() -> StarfindOptions {
    StarfindOptions {
        max_stars: usize::MAX,
        ..Default::default()
    }
}

/// Detection alone, which every other number here sits on top of.
fn detection(c: &mut Criterion) {
    let data = star_field();
    let options = uncapped();

    let mut group = c.benchmark_group("starfind_detect");
    group.throughput(criterion::Throughput::Elements((WIDTH * HEIGHT) as u64));
    group.bench_function("2048x2048", |b| {
        b.iter(|| detect_candidates(&data, WIDTH, HEIGHT, &options))
    });
    group.finish();
}

/// The whole pass, per profile. Throughput is in stars, which is the unit the
/// cost actually scales in.
fn fitting(c: &mut Criterion) {
    let data = star_field();

    let mut group = c.benchmark_group("starfind_fit");
    group.throughput(criterion::Throughput::Elements(STARS as u64));
    group.sample_size(20);

    for (name, profile) in [("gaussian", Profile::Gaussian), ("moffat", Profile::Moffat)] {
        let options = StarfindOptions {
            profile,
            ..uncapped()
        };
        group.bench_function(name, |b| {
            b.iter(|| find_stars_reported(&data, WIDTH, HEIGHT, &options, 0))
        });
    }
    group.finish();
}

/// What the star cap buys: the fitter works through candidates brightest-first
/// and stops once the cap is met, so a lower cap is strictly less work.
fn star_caps(c: &mut Criterion) {
    let data = star_field();

    let mut group = c.benchmark_group("starfind_cap");
    group.sample_size(20);
    for cap in [200usize, 2000, usize::MAX] {
        let options = StarfindOptions {
            max_stars: cap,
            ..Default::default()
        };
        let name = if cap == usize::MAX {
            "uncapped".to_string()
        } else {
            format!("{cap}_stars")
        };
        group.bench_function(name, |b| {
            b.iter(|| find_stars_reported(&data, WIDTH, HEIGHT, &options, 0))
        });
    }
    group.finish();
}

/// Detection and fitting are both `rayon` work, so they answer to the budget's
/// core count like every other stage.
fn cores(c: &mut Criterion) {
    let data = star_field();
    let options = uncapped();

    let mut group = c.benchmark_group("starfind_cores");
    group.sample_size(20);
    for cores in [1usize, 2, 4, 8, num_cores()] {
        let runtime = Runtime::new(ResourceBudget::new(4 * GB, cores)).expect("runtime");
        group.bench_function(format!("{cores}_cores"), |b| {
            b.iter(|| runtime.install(|| find_stars_reported(&data, WIDTH, HEIGHT, &options, 0)))
        });
    }
    group.finish();
}

/// A second field, the first one shifted and rotated, for the matching
/// benchmarks. Built by re-rendering rather than by moving star coordinates, so
/// the star lists being matched are genuinely independent detections.
fn shifted_field() -> (Vec<PsfStar>, Vec<PsfStar>) {
    let reference = star_field();
    let options = StarfindOptions::default();
    let reference_stars = find_stars(&reference, WIDTH, HEIGHT, &options);

    let transform = Transform::similarity(1.0, 0.004, -23.0, 41.0);
    let mut target = vec![0.02f32; WIDTH * HEIGHT];
    let inverse = transform.invert().expect("invertible");
    for star in &reference_stars {
        let p = inverse.apply(px_imageproc::register::transform::Point::new(
            star.x0, star.y0,
        ));
        let sigma = star.sx;
        let reach = (6.0 * sigma).ceil() as isize;
        let (cx, cy) = (p.x.round() as isize, p.y.round() as isize);
        for y in (cy - reach).max(0)..(cy + reach + 1).min(HEIGHT as isize) {
            for x in (cx - reach).max(0)..(cx + reach + 1).min(WIDTH as isize) {
                let (dx, dy) = (x as f64 - p.x, y as f64 - p.y);
                let q = (dx * dx + dy * dy) / (2.0 * sigma * sigma);
                if q < 30.0 {
                    target[y as usize * WIDTH + x as usize] += (star.amplitude * (-q).exp()) as f32;
                }
            }
        }
    }
    let target_stars = find_stars(&target, WIDTH, HEIGHT, &options);
    (target_stars, reference_stars)
}

/// Triangle matching and transform fitting, which is the per-frame cost of
/// registration once the stars are known.
fn matching(c: &mut Criterion) {
    let (target, reference) = shifted_field();
    let match_options = MatchOptions::default();
    let ransac_options = RansacOptions::default();

    let mut group = c.benchmark_group("register_match");
    group.sample_size(20);
    group.bench_function("triangle_match", |b| {
        b.iter(|| match_stars(&target, &reference, &match_options).expect("match"))
    });
    group.bench_function("match_and_fit", |b| {
        b.iter(|| {
            register_pair(
                &target,
                &reference,
                TransformKind::Homography,
                &match_options,
                &ransac_options,
            )
            .expect("register")
        })
    });
    group.finish();
}

/// Warp throughput per kernel, with and without clamping.
///
/// Measured at 16.8 MP rather than the 61 MP of a full frame, because the work
/// is exactly linear in pixel count and criterion wants ten samples: multiply by
/// 3.6 for a 9576x6388 frame. Clamping roughly doubles the cost, because it is
/// a second full warp plus a mask pass.
fn warping(c: &mut Criterion) {
    const SIDE: usize = 4096;
    let dims = px_imageproc::ImageDims::new(SIDE, SIDE, 1);

    // A star field the size of the bench frame, written and re-read so the
    // source is a `Frame` with a real header.
    let dir = std::env::temp_dir().join("px-imageproc-bench-warp");
    std::fs::create_dir_all(&dir).expect("bench dir");
    let path = dir.join("field.fits");
    if !path.exists() {
        let mut rng = SplitMix64(0x5741_1F7A_0000_0003);
        let mut data: Vec<f32> = (0..dims.pixel_count())
            .map(|_| (0.02 + 0.0008 * (rng.next_unit() - 0.5)) as f32)
            .collect();
        for _ in 0..4000 {
            let (x0, y0) = (SIDE as f64 * rng.next_unit(), SIDE as f64 * rng.next_unit());
            let amplitude = 0.05 + 0.6 * rng.next_unit();
            let sigma = 1.5 + 0.6 * rng.next_unit();
            let reach = (4.0 * sigma).ceil() as isize;
            let (cx, cy) = (x0.round() as isize, y0.round() as isize);
            for y in (cy - reach).max(0)..(cy + reach + 1).min(SIDE as isize) {
                for x in (cx - reach).max(0)..(cx + reach + 1).min(SIDE as isize) {
                    let (dx, dy) = (x as f64 - x0, y as f64 - y0);
                    let q = (dx * dx + dy * dy) / (2.0 * sigma * sigma);
                    if q < 16.0 {
                        data[y as usize * SIDE + x as usize] += (amplitude * (-q).exp()) as f32;
                    }
                }
            }
        }
        let header = px_fits::HeaderBuilder::primary_image(
            px_fits::BitPix::F32,
            &[SIDE as u64, SIDE as u64],
        )
        .expect("header");
        let mut writer = px_fits::FitsWriter::create(&path).expect("create");
        writer.write_image(&header, &data).expect("write");
        writer.finish().expect("finish");
    }
    let source = px_imageproc::Frame::open(&path).expect("open");
    let runtime = Runtime::new(ResourceBudget::new(4 * GB, num_cores())).expect("runtime");
    // A realistic registration transform: a few tens of pixels of dither and a
    // fraction of a degree of rotation, so no sample lands on an integer.
    let transform = Transform::similarity(1.0, 0.004, -23.5, 41.25);

    let mut group = c.benchmark_group("warp");
    group.throughput(criterion::Throughput::Elements(dims.pixel_count() as u64));
    group.sample_size(10);
    for interpolation in [
        Interpolation::Bilinear,
        Interpolation::Area,
        Interpolation::Bicubic,
        Interpolation::Lanczos4,
    ] {
        for clamp in [false, true] {
            // Clamping is a no-op for the kernels with no negative lobes, so
            // benchmarking it there would just duplicate a number.
            if clamp && !interpolation.has_negative_lobes() {
                continue;
            }
            let options = WarpOptions {
                interpolation,
                clamp,
            };
            let name = format!(
                "{}{}",
                format!("{interpolation:?}").to_lowercase(),
                if clamp { "_clamped" } else { "" }
            );
            group.bench_function(name, |b| {
                b.iter(|| warp(&source, &transform, dims, &runtime, &options).expect("warp"))
            });
        }
    }
    group.finish();
}

fn num_cores() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(4)
}

criterion_group!(
    benches, detection, fitting, star_caps, cores, matching, warping
);
criterion_main!(benches);
