//! Diagnostic for the native-vs-Siril master flat difference. Not a gate —
//! it prints a characterization so the difference can be understood rather
//! than guessed at.

use std::path::{Path, PathBuf};

use px_imageproc::Frame;

const ADU: f64 = 65535.0;

fn repo_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .and_then(Path::parent)
        .expect("repo root")
        .to_path_buf()
}

#[test]
#[cfg_attr(debug_assertions, ignore = "reads 61 MP frames; run with --release")]
fn characterize_master_flat_difference() {
    let ours = repo_root().join("data/output/pipelines/flat");
    let Ok(entries) = std::fs::read_dir(&ours) else {
        eprintln!("skipping: run the native_masters test first");
        return;
    };
    let Some(ours) = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.extension().is_some_and(|e| e == "fits"))
    else {
        eprintln!("skipping: no native master flat found");
        return;
    };
    let theirs = repo_root().join("data/reference/masters/master_flat_winsorized.fits");
    if !theirs.exists() {
        eprintln!("skipping: no Siril reference master flat");
        return;
    }

    let a = Frame::open(&ours).expect("native flat");
    let b = Frame::open(&theirs).expect("siril flat");

    // How many pixels differ by more than each threshold, in ADU.
    let thresholds = [0.01f64, 0.1, 1.0, 5.0, 20.0, 50.0, 100.0];
    let mut counts = vec![0usize; thresholds.len()];
    let mut worst: Vec<(f64, usize)> = Vec::new();

    for (i, (x, y)) in a.data().iter().zip(b.data()).enumerate() {
        let d = ((*x as f64) - (*y as f64)) * ADU;
        let m = d.abs();
        for (slot, t) in counts.iter_mut().zip(thresholds) {
            if m > t {
                *slot += 1;
            }
        }
        if m > 20.0 {
            worst.push((d, i));
        }
    }

    let total = a.data().len();
    println!("master flat difference, {total} pixels");
    for (t, c) in thresholds.iter().zip(&counts) {
        println!(
            "  > {t:>6.2} ADU: {c:>10} pixels ({:.7}%)",
            100.0 * *c as f64 / total as f64
        );
    }

    worst.sort_by(|p, q| q.0.abs().total_cmp(&p.0.abs()));
    println!("\nworst 10 pixels (delta ADU, x, y, ours ADU, siril ADU):");
    for (d, i) in worst.iter().take(10) {
        let (x, y) = (i % a.width(), i / a.width());
        println!(
            "  {:+9.2}  ({x:>5},{y:>5})  ours {:>10.2}  siril {:>10.2}",
            d,
            a.data()[*i] as f64 * ADU,
            b.data()[*i] as f64 * ADU
        );
    }

    // Are the differing pixels clustered or scattered? Clustered would point
    // at a systematic region (a block boundary); scattered at per-pixel
    // rejection decisions.
    if worst.len() > 1 {
        let rows: std::collections::HashSet<usize> =
            worst.iter().map(|(_, i)| i / a.width()).collect();
        println!(
            "\n{} pixels above 20 ADU, spread over {} distinct rows of {}",
            worst.len(),
            rows.len(),
            a.height()
        );
    }
}

/// Checks whether the differing pixels line up with block boundaries, which
/// would mean the scheduler rather than the rejection is at fault.
#[test]
#[cfg_attr(debug_assertions, ignore = "reads 61 MP frames; run with --release")]
fn block_boundaries_are_not_the_cause() {
    let ours = repo_root().join("data/output/pipelines/flat");
    let Ok(entries) = std::fs::read_dir(&ours) else {
        eprintln!("skipping: run the native_masters test first");
        return;
    };
    let Some(ours) = entries
        .filter_map(|e| e.ok().map(|e| e.path()))
        .find(|p| p.extension().is_some_and(|e| e == "fits"))
    else {
        return;
    };
    let theirs = repo_root().join("data/reference/masters/master_flat_winsorized.fits");
    if !theirs.exists() {
        return;
    }

    let a = Frame::open(&ours).expect("native flat");
    let b = Frame::open(&theirs).expect("siril flat");
    let width = a.width();

    // Per-row RMS difference. A block-boundary bug would show as a spike on
    // specific rows; a rejection difference should be uniform.
    let mut per_row: Vec<(f64, usize)> = Vec::with_capacity(a.height());
    for row in 0..a.height() {
        let start = row * width;
        let mut sum_sq = 0.0f64;
        for i in start..start + width {
            let d = ((a.data()[i] as f64) - (b.data()[i] as f64)) * ADU;
            sum_sq += d * d;
        }
        per_row.push(((sum_sq / width as f64).sqrt(), row));
    }

    let mean: f64 = per_row.iter().map(|(v, _)| v).sum::<f64>() / per_row.len() as f64;
    per_row.sort_by(|p, q| q.0.total_cmp(&p.0));
    println!("per-row RMS difference: mean {mean:.4} ADU");
    println!("worst 5 rows:");
    for (rms, row) in per_row.iter().take(5) {
        println!("  row {row:>5}: {rms:.4} ADU ({:.1}x the mean)", rms / mean);
    }
    println!("best 5 rows:");
    for (rms, row) in per_row.iter().rev().take(5) {
        println!("  row {row:>5}: {rms:.4} ADU");
    }
}

/// Siril writes bias-calibrated flats to disk through an operation that clamps
/// to [-1, 1]; the native path subtracts in flight and does not. If any
/// calibrated flat pixel would hit either clamp, that is a real structural
/// difference between the two and not a rejection-boundary effect.
#[test]
#[cfg_attr(debug_assertions, ignore = "reads 61 MP frames; run with --release")]
fn in_flight_calibration_never_reaches_a_clamp() {
    let flat_dir = repo_root().join("data/corpus-mono/flat");
    let bias_path = repo_root().join("data/reference/masters/master_bias.fits");
    if !flat_dir.is_dir() || !bias_path.exists() {
        eprintln!("skipping: needs the corpus and a Siril master bias");
        return;
    }

    let mut flats: Vec<PathBuf> = std::fs::read_dir(&flat_dir)
        .expect("read flat dir")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "fits"))
        .collect();
    flats.sort();

    let bias = Frame::open(&bias_path).expect("bias");
    let mut saturated = 0usize;
    let mut floored = 0usize;
    let mut min = f32::INFINITY;
    let mut max = f32::NEG_INFINITY;

    // A sample of frames is enough: if the clamp were being hit it would be
    // hit on every flat, since they are near-identical exposures.
    for path in flats.iter().step_by(flats.len() / 5).take(5) {
        let flat = Frame::open(path).expect("flat");
        for (f, b) in flat.data().iter().zip(bias.data()) {
            let v = f - b;
            min = min.min(v);
            max = max.max(v);
            if v > 1.0 {
                saturated += 1;
            } else if v < -1.0 {
                floored += 1;
            }
        }
    }

    println!(
        "calibrated flat range over 5 frames: [{min:.6}, {max:.6}] unit\n  \
         would be saturated: {saturated}, would be floored: {floored}"
    );
    assert_eq!(
        saturated + floored,
        0,
        "the clamp Siril applies when writing calibrated flats would fire, so it is a \
         real difference between the engines and not a rejection-boundary effect"
    );
}
