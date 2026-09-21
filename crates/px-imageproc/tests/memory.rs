//! Peak-heap assertions for the stacker (ADR 007, P3-T2).
//!
//! The memory budget is the one bound ADR 007 declares non-negotiable, and
//! `tests/budget.rs` only checks it at plan level — that the *plan* fits.
//! This checks what actually gets allocated while a stack runs.
//!
//! `dhat` allows one live profiler per process and its stats are
//! process-global, so every test here holds a lock for its whole window.

#[global_allocator]
static ALLOC: dhat::Alloc = dhat::Alloc;

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use px_fits::card::Value;
use px_fits::{BitPix, FitsWriter, HeaderBuilder};
use px_imageproc::FrameSet;
use px_imageproc::runtime::{ResourceBudget, Runtime};
use px_imageproc::stack::{Bounds, Method, Rejection, StackOptions, stack};

static DHAT_LOCK: Mutex<()> = Mutex::new(());

const MIB: u64 = 1 << 20;

fn build(dir: &Path, frames: usize, width: usize, height: usize) -> Vec<PathBuf> {
    let _ = std::fs::remove_dir_all(dir);
    std::fs::create_dir_all(dir).expect("create dir");
    (0..frames)
        .map(|f| {
            let path = dir.join(format!("f{f:03}.fits"));
            let data: Vec<i16> = (0..width * height)
                .map(|i| ((1000 + (i + f) % 200) as i32 - 32768) as i16)
                .collect();
            let header = HeaderBuilder::primary_image(BitPix::I16, &[width as u64, height as u64])
                .expect("header")
                .card("BZERO", Value::Integer(32768), None)
                .expect("bzero");
            let mut w = FitsWriter::create(&path).expect("create");
            w.write_image(&header, &data).expect("write");
            w.finish().expect("finish");
            path
        })
        .collect()
}

/// What the plan says the run is allowed to hold at once.
fn planned_bytes(
    width: usize,
    height: usize,
    channels: usize,
    frames: usize,
    cores: usize,
    largest_block_height: usize,
    maps: usize,
) -> u64 {
    let pixels = (width * height * channels) as u64;
    let result = pixels * 4;
    let rejection_maps = maps as u64 * pixels * 2;
    let band = frames as u64 * (largest_block_height * width) as u64 * 4;
    // Per worker: the band, the per-pixel stack, and the rejection scratch.
    let per_worker = band + (frames as u64 * 4) * 3;
    result + rejection_maps + cores as u64 * per_worker
}

#[test]
fn a_stack_stays_inside_its_planned_allocation() {
    let _guard = DHAT_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let (width, height, frames, cores) = (512usize, 512usize, 20usize, 4usize);
    let dir = std::env::temp_dir().join("px-imageproc-memory/plan");
    let paths = build(&dir, frames, width, height);
    let set = FrameSet::open(&paths).expect("open");

    // 6 MiB forces a real multi-block plan for a 1 MiB result.
    let runtime = Runtime::new(ResourceBudget::new(6 * MIB, cores)).expect("runtime");
    let plan = runtime
        .plan_blocks(&set.dims(), frames, 4, 0, 0)
        .expect("plan");

    let profiler = dhat::Profiler::new_heap();
    let result = stack(&set, &runtime, StackOptions::new(Method::Median)).expect("stack");
    let stats = dhat::HeapStats::get();
    drop(profiler);

    let allowance = planned_bytes(
        width,
        height,
        1,
        frames,
        cores,
        plan.largest_block_height,
        0,
    );
    // Slack for read scratch inside px-fits, Vec growth, and the harness
    // itself. Generous, but an order of magnitude below the failure this
    // guards against — a stacker that quietly holds whole frames.
    let ceiling = allowance + 4 * MIB;

    println!(
        "peak heap {} bytes, planned {allowance}, ceiling {ceiling} ({} blocks, tallest {} rows)",
        stats.max_bytes, result.block_count, plan.largest_block_height
    );

    assert!(
        (stats.max_bytes as u64) <= ceiling,
        "peak heap {} exceeded the planned {allowance} (+slack) — is the stacker holding whole frames?",
        stats.max_bytes
    );
    assert!(
        (stats.max_bytes as u64) <= 6 * MIB + 4 * MIB,
        "peak heap {} exceeded the 6 MiB budget by more than the harness slack",
        stats.max_bytes
    );
}

#[test]
fn peak_heap_does_not_grow_with_the_sequence_length() {
    // The point of block scheduling: doubling the frame count must not double
    // what is resident, because a longer sequence gets smaller blocks.
    let _guard = DHAT_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let (width, height, cores) = (512usize, 512usize, 2usize);
    let mut peaks = Vec::new();

    for frames in [10usize, 40] {
        let dir = std::env::temp_dir().join(format!("px-imageproc-memory/len{frames}"));
        let paths = build(&dir, frames, width, height);
        let set = FrameSet::open(&paths).expect("open");
        let runtime = Runtime::new(ResourceBudget::new(6 * MIB, cores)).expect("runtime");

        let profiler = dhat::Profiler::new_heap();
        let _ = stack(&set, &runtime, StackOptions::new(Method::Median)).expect("stack");
        let stats = dhat::HeapStats::get();
        drop(profiler);
        peaks.push(stats.max_bytes as u64);
    }

    println!(
        "peak heap: 10 frames {} bytes, 40 frames {} bytes",
        peaks[0], peaks[1]
    );
    assert!(
        peaks[1] < peaks[0] * 2,
        "4x the frames pushed peak heap from {} to {} — blocks are not shrinking",
        peaks[0],
        peaks[1]
    );
}

#[test]
fn rejection_maps_are_charged_against_the_budget_not_added_to_it() {
    // Naively, asking for rejection maps should raise peak heap by two u16
    // images. It does not — and that is the budgeter working. The maps are
    // charged before the block plan is drawn, so the bands shrink to make
    // room and the total stays put. The assertion that matters is therefore
    // not "maps cost more" but "maps do not breach the budget".
    let _guard = DHAT_LOCK.lock().unwrap_or_else(|e| e.into_inner());

    let (width, height, frames, cores) = (512usize, 512usize, 20usize, 2usize);
    let dir = std::env::temp_dir().join("px-imageproc-memory/maps");
    let paths = build(&dir, frames, width, height);
    let set = FrameSet::open(&paths).expect("open");
    let budget = 8 * MIB;
    let runtime = Runtime::new(ResourceBudget::new(budget, cores)).expect("runtime");

    let method = Method::Mean(Rejection::Sigma(Bounds::symmetric(3.0)));

    let profiler = dhat::Profiler::new_heap();
    let _ = stack(&set, &runtime, StackOptions::new(method)).expect("stack");
    let without = dhat::HeapStats::get().max_bytes as u64;
    drop(profiler);

    let profiler = dhat::Profiler::new_heap();
    let result = stack(
        &set,
        &runtime,
        StackOptions::new(method).rejection_maps(true),
    )
    .expect("stack");
    let with = dhat::HeapStats::get().max_bytes as u64;
    drop(profiler);

    let map_bytes = 2 * (width * height) as u64 * 2;
    println!(
        "peak heap: without maps {without}, with maps {with} (maps are {map_bytes} bytes, \
         budget {budget})"
    );

    assert!(result.rejection_map_low.is_some() && result.rejection_map_high.is_some());
    assert!(
        with <= budget + 2 * MIB,
        "peak heap {with} breached the {budget} budget when rejection maps were requested"
    );
    assert!(
        without <= budget + 2 * MIB,
        "peak heap {without} breached the {budget} budget"
    );
    // The maps cannot cost more than they are, because the plan pays for them
    // out of the band buffers.
    assert!(
        with < without + map_bytes,
        "rejection maps added {} bytes, more than the {map_bytes} they occupy — \
         the budget is not compensating",
        with.saturating_sub(without)
    );
}
