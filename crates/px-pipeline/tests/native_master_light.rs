//! The native `CreateMasterLight` pipeline on synthetic frames
//! (ADR 007, P11-T1 / P11-T3 / P11-T4).
//!
//! Synthetic rather than corpus frames, so the test runs everywhere and in
//! seconds, and so the *truth* is known: the stars are placed, the dither is
//! known, and the sky level per frame is known. Parity against Siril over real
//! frames is `master_light_parity.rs`.

use std::path::{Path, PathBuf};

use px_fits::{BitPix, FitsWriter, HeaderBuilder};
use px_imageproc::register::psf::fwhm_stats;
use px_imageproc::register::starfind::find_stars;
use px_imageproc::{Frame, StarfindOptions};
use px_pipeline::project::master_light::CreateMasterLightPipeline;
use px_pipeline::{NativeRuntime, PipelineReporter, RunIn};
use siril_sys::FitsExt;

const WIDTH: usize = 420;
const HEIGHT: usize = 340;

struct Quiet;

impl PipelineReporter for Quiet {
    fn step_started(&self, _message: &str) -> usize {
        0
    }
    fn step_ended(&self, _id: usize, _message: &str, _success: bool) {}
}

struct SplitMix64(u64);

impl SplitMix64 {
    fn next_unit(&mut self) -> f64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^= z >> 31;
        (z >> 11) as f64 / (1u64 << 53) as f64
    }
}

/// A field of stars in sky coordinates.
/// A field of stars in sky coordinates, confined to the left half of the frame.
///
/// Confined deliberately: the right side is then a genuinely blank patch, and
/// blank sky is the only place a background-noise measurement means what it says.
/// On a star-dense field the estimator measures the wings of stars — which are
/// identical in every frame and so do not average down — and a stack looks as
/// noisy as a single frame when it is not.
fn sky(count: usize, seed: u64) -> Vec<(f64, f64, f64)> {
    let mut rng = SplitMix64(seed);
    (0..count)
        .map(|_| {
            (
                0.10 * WIDTH as f64 + 0.45 * WIDTH as f64 * rng.next_unit(),
                0.12 * HEIGHT as f64 + 0.76 * HEIGHT as f64 * rng.next_unit(),
                0.2 + 0.5 * rng.next_unit(),
            )
        })
        .collect()
}

/// The RMS deviation from the median over a star-free patch, well clear of the
/// warp's unexposed border.
fn blank_patch_noise(frame: &Frame) -> f64 {
    let (x0, x1) = (frame.width() * 70 / 100, frame.width() * 95 / 100);
    let (y0, y1) = (frame.height() * 20 / 100, frame.height() * 80 / 100);
    let mut values: Vec<f32> = Vec::new();
    for y in y0..y1 {
        for x in x0..x1 {
            values.push(frame.get(0, x, y));
        }
    }
    let median = px_imageproc::stats::median(&values) as f64;
    let sum_squared: f64 = values
        .iter()
        .map(|v| (*v as f64 - median) * (*v as f64 - median))
        .sum();
    (sum_squared / values.len() as f64).sqrt()
}

/// Writes one light frame: the sky shifted by `(dx, dy)`, on a background of
/// `sky_level`, with a hot pixel and a cosmic ray for rejection to remove.
#[allow(clippy::too_many_arguments)]
fn write_light(
    dir: &Path,
    index: usize,
    stars: &[(f64, f64, f64)],
    dx: f64,
    dy: f64,
    sky_level: f32,
    transparency: f32,
    exposure: f64,
) -> PathBuf {
    let path = dir.join(format!("pp_light_{index:03}.fits"));
    let mut rng = SplitMix64(0xC0DE_0000 + index as u64);
    let mut data: Vec<f32> = (0..WIDTH * HEIGHT)
        .map(|_| sky_level + 0.0009 * (rng.next_unit() - 0.5) as f32)
        .collect();

    let sigma = 1.7f64;
    for &(x, y, amplitude) in stars {
        let (cx, cy) = (x - dx, y - dy);
        let reach = (5.0 * sigma).ceil() as isize;
        for row in (cy.round() as isize - reach).max(0)
            ..(cy.round() as isize + reach + 1).min(HEIGHT as isize)
        {
            for column in (cx.round() as isize - reach).max(0)
                ..(cx.round() as isize + reach + 1).min(WIDTH as isize)
            {
                let (ddx, ddy) = (column as f64 - cx, row as f64 - cy);
                let q = (ddx * ddx + ddy * ddy) / (2.0 * sigma * sigma);
                if q < 25.0 {
                    data[row as usize * WIDTH + column as usize] +=
                        transparency * (amplitude * (-q).exp()) as f32;
                }
            }
        }
    }

    // One hot pixel in the same place every frame — which rejection cannot
    // remove, and which the *registration* does, by moving it to a different
    // sky position in each frame — and one cosmic ray unique to this frame.
    data[200 * WIDTH + 100] = 0.95;
    data[(50 + index * 7) % HEIGHT * WIDTH + (31 + index * 13) % WIDTH] = 0.99;

    let header = HeaderBuilder::primary_image(BitPix::F32, &[WIDTH as u64, HEIGHT as u64])
        .expect("header")
        .card("EXPTIME", px_fits::card::Value::Float(exposure), None)
        .expect("exptime")
        .card(
            "OBJECT",
            px_fits::card::Value::String("SYNTH 1".to_string()),
            None,
        )
        .expect("object")
        .card(
            "FILTER",
            px_fits::card::Value::String("Ha".to_string()),
            None,
        )
        .expect("filter");
    let mut writer = FitsWriter::create(&path).expect("create");
    writer.write_image(&header, &data).expect("write");
    writer.finish().expect("finish");
    path
}

/// A session: ten frames, dithered, with the sky brightening and transparency
/// dropping through the run — which is what normalization has to absorb.
fn session(dir: &Path, frames: usize) -> Vec<PathBuf> {
    let stars = sky(150, 0xA11CE);
    (0..frames)
        .map(|i| {
            let angle = i as f64 * 0.7;
            write_light(
                dir,
                i,
                &stars,
                6.0 * angle.cos(),
                6.0 * angle.sin(),
                0.02 + 0.004 * i as f32,
                1.0 - 0.02 * i as f32,
                120.0,
            )
        })
        .collect()
}

fn pipeline(lights: &Path, out: &Path) -> CreateMasterLightPipeline {
    CreateMasterLightPipeline::builder()
        .ext(FitsExt::FITS)
        .light_folders(vec![lights.to_path_buf()])
        .name("synth".to_string())
        .out_folder(out.to_path_buf())
        .build()
}

#[tokio::test]
async fn stacks_a_dithered_session_from_memory() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lights = dir.path().join("lights");
    std::fs::create_dir_all(&lights).expect("lights dir");
    let out = dir.path().join("out");
    let frames = session(&lights, 10);

    let master = pipeline(&lights, &out)
        .run(Quiet, NativeRuntime::default().use_memory_fixed_gb(2.0))
        .await
        .expect("master light");

    assert_eq!(master.frame_count, frames.len());
    assert_eq!(master.target_name, "SYNTH 1");
    assert_eq!(master.filter.as_deref(), Some("Ha"));
    assert!(
        (master.exposure - 1200.0).abs() < 1e-6,
        "total exposure {} should be the sum of the frames'",
        master.exposure
    );

    let stacked = Frame::open(&master.path).expect("open master");
    assert_eq!(stacked.width(), WIDTH);
    assert_eq!(stacked.height(), HEIGHT);

    // Signal-to-noise, not noise: the pipeline finishes with Siril's
    // `-output_norm`, which rescales the result into the unit range, so the
    // absolute noise of a master and of a single frame are in different units.
    // What stacking buys is SNR, and ten frames should buy most of the sqrt(10)
    // available.
    let snr = |frame: &Frame| -> f64 {
        let noise = blank_patch_noise(frame);
        let stars = find_stars(
            frame.channel(0),
            frame.width(),
            frame.height(),
            &StarfindOptions::default(),
        );
        let brightest = stars.iter().map(|s| s.amplitude).fold(0.0f64, f64::max);
        brightest / noise
    };
    let single = Frame::open(&frames[0]).expect("open a light");
    let (single_snr, stacked_snr) = (snr(&single), snr(&stacked));
    println!(
        "SNR: single {single_snr:.0}, stacked {stacked_snr:.0} ({:.2}x)",
        stacked_snr / single_snr
    );
    assert!(
        stacked_snr > 2.2 * single_snr,
        "stacked SNR {stacked_snr:.0} vs single-frame {single_snr:.0}: only {:.2}x of the \
         3.16x available from 10 frames",
        stacked_snr / single_snr
    );

    // And the stars must still be stars, not smeared by a bad transform.
    let stars = find_stars(
        stacked.channel(0),
        WIDTH,
        HEIGHT,
        &StarfindOptions::default(),
    );
    let stats = fwhm_stats(&stars).expect("stars in the stack");
    assert!(stats.count > 80, "only {} stars in the stack", stats.count);
    assert!(
        stats.median < 2.0 * 1.7 * 2.3548,
        "stacked FWHM {:.2} px is more than twice the rendered one",
        stats.median
    );
    assert!(stats.mean_roundness > 0.85, "{:?}", stats.mean_roundness);
}

#[tokio::test]
async fn the_spill_path_gives_the_same_master_as_the_memory_path() {
    // ADR 007 P11-T3: a budget too small to hold the warped frames must spill
    // them to a temporary sequence and produce the same image.
    let dir = tempfile::tempdir().expect("tempdir");
    let lights = dir.path().join("lights");
    std::fs::create_dir_all(&lights).expect("lights dir");
    session(&lights, 6);

    let roomy = dir.path().join("roomy");
    let memory = pipeline(&lights, &roomy)
        .run(Quiet, NativeRuntime::default().use_memory_fixed_gb(2.0))
        .await
        .expect("in-memory master");

    let cramped = dir.path().join("cramped");
    // 6 frames x 420 x 340 x 4 bytes is 3.4 MB of warped frames; 4 MB of budget
    // leaves 2 MB for them, so they must spill.
    let spilled = pipeline(&lights, &cramped)
        .run(
            Quiet,
            NativeRuntime::default().use_memory_bytes(4 * 1024 * 1024),
        )
        .await
        .expect("spilled master");

    let a = Frame::open(&memory.path).expect("open");
    let b = Frame::open(&spilled.path).expect("open");
    assert_eq!(a.dims(), b.dims());
    let worst = a
        .data()
        .iter()
        .zip(b.data())
        .map(|(x, y)| (x - y).abs())
        .fold(0.0f32, f32::max);
    // Not bit-identical: the spill path round-trips each warped frame through
    // f32 FITS, and the band sizes differ, so sums associate differently.
    assert!(worst < 1e-5, "the two paths differ by {worst}");
}

#[tokio::test]
async fn background_extraction_is_refused_rather_than_ignored() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lights = dir.path().join("lights");
    std::fs::create_dir_all(&lights).expect("lights dir");
    session(&lights, 3);

    let result = CreateMasterLightPipeline::builder()
        .ext(FitsExt::FITS)
        .light_folders(vec![lights.clone()])
        .name("synth".to_string())
        .background_extract(true)
        .out_folder(dir.path().join("out"))
        .build()
        .run(Quiet, NativeRuntime::default())
        .await;

    let error = result.expect_err("background extraction is not implemented natively");
    let message = error.to_string();
    assert!(
        message.contains("native") && message.contains("background"),
        "{message}"
    );
}

#[tokio::test]
async fn a_clouded_frame_is_dropped_and_the_rest_still_stack() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lights = dir.path().join("lights");
    std::fs::create_dir_all(&lights).expect("lights dir");
    let stars = sky(150, 0xB0B);
    for i in 0..5 {
        write_light(
            &lights,
            i,
            &stars,
            3.0 * i as f64,
            -2.0 * i as f64,
            0.02,
            1.0,
            60.0,
        );
    }
    // Sky only: no stars to register on.
    write_light(&lights, 99, &[], 0.0, 0.0, 0.02, 1.0, 60.0);

    let master = pipeline(&lights, &dir.path().join("out"))
        .run(Quiet, NativeRuntime::default().use_memory_fixed_gb(2.0))
        .await
        .expect("master light");

    assert_eq!(master.frame_count, 5, "the clouded frame must be dropped");
    assert!(
        (master.exposure - 300.0).abs() < 1e-6,
        "{}",
        master.exposure
    );
}

#[tokio::test]
async fn a_single_frame_is_not_a_stack() {
    let dir = tempfile::tempdir().expect("tempdir");
    let lights = dir.path().join("lights");
    std::fs::create_dir_all(&lights).expect("lights dir");
    session(&lights, 1);

    let result = pipeline(&lights, &dir.path().join("out"))
        .run(Quiet, NativeRuntime::default())
        .await;
    assert!(
        result.is_err(),
        "one frame cannot be registered to anything"
    );
}
