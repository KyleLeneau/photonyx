//! Frame statistics (ADR 007, Phase 2).
//!
//! These are the estimators the rest of the engine is built on: rejection
//! thresholds are expressed in sigma, normalization coefficients come from the
//! robust location/scale pair, and parity tests are stated in units of the
//! background noise.
//!
//! Two conventions run through this module and are worth stating once.
//!
//! **Zero is null.** A zero sample counts as "no data", not as "a black
//! pixel" — it is what a warp leaves outside the source footprint and what an
//! unexposed sensor region reads. Including those in a background estimate
//! would drag it toward zero in proportion to how much of the frame is empty.
//! NaN is treated the same way.
//!
//! **Sigma is the population RMS**, divided by the sample count rather than
//! `n - 1`. Frames have millions of pixels, so the distinction is numerically
//! irrelevant, but it is what the reference implementation uses and the
//! rejection thresholds are calibrated against.
//!
//! The robust scale estimator is the biweight midvariance under an iterative
//! clip, after Randal & Rousseeuw's work on robust scale; the 0.991 factor
//! below is the consistency correction that makes it comparable to a standard
//! deviation for normally distributed data.

use rayon::prelude::*;

/// Samples per reduction chunk.
///
/// Reductions are chunked at a *fixed* size rather than split across however
/// many threads happen to be available, so a sum is associated the same way
/// every time. Statistics that shifted in the last bits when the core count
/// changed would make every downstream parity test irreproducible.
const CHUNK: usize = 16_384;

/// Consistency factor making the biweight scale comparable to a standard
/// deviation on normal data.
const BIWEIGHT_CONSISTENCY: f64 = 0.991;

/// Half-width, in MADs, of the window the robust estimators clip to.
const CLIP_MADS: f32 = 6.0;

/// Sigma multiplier for the noise estimator's clipping iterations.
const NOISE_SIGMA_CLIP: f64 = 5.0;

/// Clipping iterations the noise estimator performs per row.
const NOISE_ITERATIONS: usize = 3;

/// `1/sqrt(2)`: successive differences of independent samples have twice the
/// variance of the samples themselves.
const INV_SQRT_2: f64 = std::f64::consts::FRAC_1_SQRT_2;

/// Scale factor making the MAD a consistent estimator of sigma on normal data.
pub const MAD_TO_SIGMA: f64 = 1.4826;

#[inline]
fn is_null(v: f32) -> bool {
    v == 0.0 || v.is_nan()
}

/// Copies out the samples that carry data.
pub fn non_null(data: &[f32]) -> Vec<f32> {
    data.iter().copied().filter(|v| !is_null(*v)).collect()
}

/// Sample count, mean, and population sigma over the non-null samples.
///
/// Returns `(0, 0.0, 0.0)` for an empty or entirely null input.
pub fn mean_sigma(data: &[f32]) -> (usize, f64, f64) {
    let (count, sum, sum_sq) = data
        .par_chunks(CHUNK)
        .map(|chunk| {
            let mut count = 0usize;
            let mut sum = 0.0f64;
            let mut sum_sq = 0.0f64;
            for &v in chunk {
                if !is_null(v) {
                    let x = v as f64;
                    count += 1;
                    sum += x;
                    sum_sq += x * x;
                }
            }
            (count, sum, sum_sq)
        })
        .reduce(
            || (0usize, 0.0f64, 0.0f64),
            |a, b| (a.0 + b.0, a.1 + b.1, a.2 + b.2),
        );

    match count {
        0 => (0, 0.0, 0.0),
        1 => (1, sum, 0.0),
        n => {
            let mean = sum / n as f64;
            // Clamped because catastrophic cancellation can push this a hair
            // below zero when every sample is nearly identical.
            let var = (sum_sq / n as f64 - mean * mean).max(0.0);
            (n, mean, var.sqrt())
        }
    }
}

/// Smallest and largest non-null samples, or `None` if there are none.
pub fn min_max(data: &[f32]) -> Option<(f32, f32)> {
    data.par_chunks(CHUNK)
        .map(|chunk| {
            chunk.iter().copied().filter(|v| !is_null(*v)).fold(
                None,
                |acc: Option<(f32, f32)>, v| match acc {
                    None => Some((v, v)),
                    Some((lo, hi)) => Some((lo.min(v), hi.max(v))),
                },
            )
        })
        .reduce(
            || None,
            |a, b| match (a, b) {
                (None, x) | (x, None) => x,
                (Some((a0, a1)), Some((b0, b1))) => Some((a0.min(b0), a1.max(b1))),
            },
        )
}

/// Median of `data`, reordering it in place. `data` must not contain NaN.
///
/// Even-length input averages the two central samples.
pub fn median_in_place(data: &mut [f32]) -> f32 {
    let n = data.len();
    match n {
        0 => 0.0,
        1 => data[0],
        _ if n % 2 == 1 => select_nth(data, n / 2),
        _ => {
            let hi = select_nth(data, n / 2);
            // Everything below index n/2 is now <= hi, so the lower central
            // sample is the largest of them — no second selection needed.
            let lo = data[..n / 2]
                .iter()
                .copied()
                .fold(f32::NEG_INFINITY, f32::max);
            (lo + hi) * 0.5
        }
    }
}

/// Median over the non-null samples, leaving `data` untouched.
pub fn median(data: &[f32]) -> f32 {
    if data.len() < PARALLEL_SELECT_MIN {
        let mut kept = non_null(data);
        return median_in_place(&mut kept);
    }
    median_by(data, |v| v)
}

/// Median absolute deviation about `center`, over the non-null samples.
///
/// Nullness is decided on the sample, not on its deviation: a pixel sitting
/// exactly at `center` has a deviation of zero and is data, not absence of it.
pub fn mad(data: &[f32], center: f32) -> f32 {
    if data.len() < PARALLEL_SELECT_MIN {
        let mut devs: Vec<f32> = data
            .iter()
            .copied()
            .filter(|v| !is_null(*v))
            .map(|v| (v - center).abs())
            .collect();
        return median_in_place(&mut devs);
    }
    median_by(data, |v| (v - center).abs())
}

/// Non-null samples below which the serial quickselect is used instead of the
/// counting passes.
///
/// The parallel path costs three linear sweeps and a pair of 512 KB
/// reductions; below roughly a quarter of a million samples that is more
/// arithmetic than copy-and-partition, and the copy is small enough not to
/// matter. Both paths compute the same number, so this is purely a speed
/// choice.
const PARALLEL_SELECT_MIN: usize = 1 << 18;

/// Bits of the order key each counting pass resolves. Two passes of 16 cover
/// the whole 32-bit key, so the second pass lands on an exact value rather
/// than on a range that would have to be interpolated.
const RADIX_BITS: u32 = 16;
const RADIX_BINS: usize = 1 << RADIX_BITS;

/// Maps a finite `f32` to a `u32` that compares in the same order.
///
/// IEEE-754 was designed so that the bit patterns of non-negative floats
/// increase with the value; negatives increase the other way, and sort after
/// rather than before. Flipping every bit of a negative reverses it and moves
/// it below zero, and setting the top bit of a non-negative moves it above —
/// after which plain unsigned comparison is float comparison, and the value
/// can be counted with integer arithmetic.
#[inline]
fn order_key(v: f32) -> u32 {
    let bits = v.to_bits();
    if bits & 0x8000_0000 != 0 {
        !bits
    } else {
        bits | 0x8000_0000
    }
}

/// Inverse of [`order_key`]. The two branches are disjoint on the top bit, so
/// the key says which one produced it.
#[inline]
fn from_order_key(key: u32) -> f32 {
    f32::from_bits(if key & 0x8000_0000 != 0 {
        key & 0x7FFF_FFFF
    } else {
        !key
    })
}

/// Exact median of `map(v)` over the non-null samples, by counting rather than
/// by partitioning.
///
/// Quickselect needs a copy it is allowed to reorder and is inherently serial:
/// each partition's bounds depend on the last. On a 61 MP channel that is a
/// 244 MB allocation and about half a second on one core, which the profile of
/// P12-T1 found to be 82% of a star-finding pass. Counting the samples into
/// buckets of their order key has neither problem — every sample is
/// independent, so the passes parallelize, and nothing is copied.
///
/// The result is the same number, not an approximation: the key is an exact
/// order-preserving encoding, and the second pass resolves the remaining 16
/// bits of it.
fn median_by(data: &[f32], map: impl Fn(f32) -> f32 + Sync) -> f32 {
    let high = tally(data, &map, |key| Some((key >> RADIX_BITS) as usize));
    let n: u64 = high.iter().sum();
    if n == 0 {
        return 0.0;
    }

    // An even count averages the two central samples, which is what the serial
    // median this replaces does.
    let upper = n / 2;
    let lower = if n % 2 == 1 { upper } else { upper - 1 };

    let pick = |rank: u64| -> f32 {
        let (bucket, below) = locate(&high, rank);
        let low = tally(data, &map, |key| {
            ((key >> RADIX_BITS) as usize == bucket)
                .then_some((key & (RADIX_BINS as u32 - 1)) as usize)
        });
        let (offset, _) = locate(&low, rank - below);
        from_order_key(((bucket as u32) << RADIX_BITS) | offset as u32)
    };

    let hi = pick(upper);
    if lower == upper {
        hi
    } else {
        (pick(lower) + hi) * 0.5
    }
}

/// Counts non-null samples into [`RADIX_BINS`] buckets.
///
/// `select` returns the bucket a sample's order key belongs to, or `None` when
/// this pass is not counting it — which is how the second pass restricts
/// itself to one bucket of the first.
fn tally(
    data: &[f32],
    map: &(impl Fn(f32) -> f32 + Sync),
    select: impl Fn(u32) -> Option<usize> + Sync,
) -> Vec<u64> {
    // Split into one chunk per thread rather than into fixed-size chunks: each
    // chunk allocates a 512 KB bucket array, so the chunk count is the cost
    // here. Counts are integers and add exactly, so unlike the floating-point
    // reductions above this needs no fixed chunking to stay reproducible.
    let workers = rayon::current_num_threads().max(1);
    let chunk = data.len().div_ceil(workers).max(1);
    data.par_chunks(chunk)
        .map(|chunk| {
            let mut bins = vec![0u64; RADIX_BINS];
            for &v in chunk {
                // Nullness is a property of the sample, so it is tested before
                // `map` rather than after: a deviation of exactly zero is a
                // real measurement.
                if is_null(v) {
                    continue;
                }
                if let Some(bin) = select(order_key(map(v))) {
                    bins[bin] += 1;
                }
            }
            bins
        })
        .reduce(
            || vec![0u64; RADIX_BINS],
            |mut a, b| {
                for (x, y) in a.iter_mut().zip(b) {
                    *x += y;
                }
                a
            },
        )
}

/// The bucket holding the `rank`-th smallest sample, and how many samples fall
/// below that bucket.
fn locate(bins: &[u64], rank: u64) -> (usize, u64) {
    let mut below = 0u64;
    for (bin, &count) in bins.iter().enumerate() {
        if below + count > rank {
            return (bin, below);
        }
        below += count;
    }
    // Only reachable if `rank` is past the end, which the callers do not do.
    (bins.len() - 1, below)
}

/// Biweight midvariance of `data` about `center`, with `mad` setting the
/// rejection width. Returns 0 when `mad` is not positive.
pub fn biweight_midvariance(data: &[f32], center: f32, mad: f32) -> f64 {
    if mad <= 0.0 {
        return 0.0;
    }
    let factor = 1.0 / (9.0 * mad);

    let (up, down) = data
        .par_chunks(CHUNK)
        .map(|chunk| {
            let mut up = 0.0f64;
            let mut down = 0.0f64;
            for &v in chunk {
                let d = v - center;
                let y = d * factor;
                // Samples beyond 9 MADs get zero weight rather than a
                // negative one.
                let y2 = if y.abs() < 1.0 { y * y } else { 1.0 };
                let w = 1.0 - y2;
                up += ((d * w * w) as f64).powi(2);
                down += (w * (1.0 - 5.0 * y2)) as f64;
            }
            (up, down)
        })
        .reduce(|| (0.0, 0.0), |a, b| (a.0 + b.0, a.1 + b.1));

    if down == 0.0 {
        0.0
    } else {
        data.len() as f64 * (up / (down * down))
    }
}

/// Robust location and scale, from an iterated clip about the median.
///
/// Samples outside `median ± 6·MAD` are dropped, then location is the median
/// of what is left and scale the biweight midvariance about it. Returns `None`
/// when the clip empties the data or the surviving samples are all identical —
/// both mean there is no spread to estimate.
pub fn location_scale(data: &[f32], median_hint: f32, mad_hint: f32) -> Option<(f64, f64)> {
    let mut values: Vec<f32> = data.iter().copied().filter(|&v| !is_null(v)).collect();
    clipped_location_scale(&mut values, median_hint, mad_hint)
}

/// The body of [`location_scale`], working in a caller-owned buffer.
///
/// `values` is clipped in place, so a caller that already holds it —
/// [`ChannelStats::compute`] — pays no further allocation.
fn clipped_location_scale(
    values: &mut Vec<f32>,
    median_hint: f32,
    mad_hint: f32,
) -> Option<(f64, f64)> {
    let lo = median_hint - CLIP_MADS * mad_hint;
    let hi = median_hint + CLIP_MADS * mad_hint;

    values.retain(|&v| !is_null(v) && v >= lo && v <= hi);
    if values.is_empty() {
        return None;
    }

    let location = median(values);
    // `mad` rather than a second buffer of deviations: it counts the mapped
    // values without materializing them, and a deviation of exactly zero — a
    // sample sitting on the location — is data it must not skip.
    let spread = mad(values, location);
    if spread <= 0.0 {
        return None;
    }

    let scale = biweight_midvariance(values, location, spread).sqrt() * BIWEIGHT_CONSISTENCY;
    Some((location as f64, scale))
}

/// The cheap location/scale pair: the median itself, and `1.5 × MAD` as a
/// stand-in for the biweight scale.
///
/// Accurate to a few percent on real frames and a great deal faster, which is
/// what makes it worth having for the normalization of long sequences.
pub fn lite_location_scale(median: f32, mad: f32) -> (f64, f64) {
    (median as f64, 1.5 * mad as f64)
}

/// Background noise, estimated from first-order differences along each row.
///
/// Within a row, successive non-null samples are differenced; the spread of
/// those differences is dominated by noise rather than by the sky gradient,
/// which varies far more slowly than pixel to pixel. Each row's spread is
/// sigma-clipped to shed stars, and the frame's estimate is the median across
/// rows — so a handful of star-crowded rows cannot move it.
///
/// `data` is one channel, row-major.
pub fn background_noise(data: &[f32], width: usize, height: usize) -> f64 {
    // Fewer than three samples per row leaves nothing to difference.
    if width < 3 || height == 0 {
        return 0.0;
    }

    let mut row_sigmas: Vec<f64> = (0..height)
        .into_par_iter()
        .filter_map(|row| row_noise(&data[row * width..(row + 1) * width]))
        .collect();

    match row_sigmas.len() {
        0 => 0.0,
        1 => INV_SQRT_2 * row_sigmas[0],
        n => {
            row_sigmas.sort_by(f64::total_cmp);
            let median = (row_sigmas[(n - 1) / 2] + row_sigmas[n / 2]) * 0.5;
            INV_SQRT_2 * median
        }
    }
}

/// Clipped spread of one row's first-order differences, or `None` if the row
/// has too little data to estimate from.
fn row_noise(row: &[f32]) -> Option<f64> {
    let mut diffs: Vec<f32> = Vec::with_capacity(row.len());
    let mut previous: Option<f32> = None;
    for &v in row {
        if is_null(v) {
            continue;
        }
        if let Some(p) = previous {
            diffs.push(p - v);
        }
        previous = Some(v);
    }
    if diffs.len() < 2 {
        return None;
    }

    let (_, mut mean, mut sigma) = mean_sigma_serial(&diffs);
    if sigma > 0.0 {
        for _ in 0..NOISE_ITERATIONS {
            let before = diffs.len();
            let limit = (NOISE_SIGMA_CLIP * sigma) as f32;
            let center = mean as f32;
            diffs.retain(|d| (d - center).abs() < limit);
            if diffs.len() == before || diffs.is_empty() {
                break;
            }
            let next = mean_sigma_serial(&diffs);
            mean = next.1;
            sigma = next.2;
        }
    }
    Some(sigma)
}

/// Row-sized mean/sigma. Rows are far too short to be worth handing to a
/// thread pool, and `background_noise` is already parallel over rows.
fn mean_sigma_serial(data: &[f32]) -> (usize, f64, f64) {
    let mut count = 0usize;
    let mut sum = 0.0f64;
    let mut sum_sq = 0.0f64;
    for &v in data {
        if !is_null(v) {
            let x = v as f64;
            count += 1;
            sum += x;
            sum_sq += x * x;
        }
    }
    match count {
        0 => (0, 0.0, 0.0),
        1 => (1, sum, 0.0),
        n => {
            let mean = sum / n as f64;
            (n, mean, (sum_sq / n as f64 - mean * mean).max(0.0).sqrt())
        }
    }
}

/// Every estimator for one channel, computed in one pass over the data.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ChannelStats {
    /// Non-null samples the estimates are drawn from.
    pub count: usize,
    pub min: f32,
    pub max: f32,
    pub mean: f64,
    pub sigma: f64,
    pub median: f32,
    pub mad: f32,
    pub background_noise: f64,
    /// Robust location, or the median when the robust estimate is degenerate.
    pub location: f64,
    /// Robust scale, or `1.4826 × MAD` when the robust estimate is degenerate.
    pub scale: f64,
}

impl ChannelStats {
    /// Computes every estimator for one channel, row-major.
    ///
    /// Peak extra allocation is one buffer the size of the non-null data,
    /// which the clipped biweight needs because it reorders and shrinks what
    /// it is given. The median and the MAD are counted rather than selected
    /// (see [`median`]) and copy nothing, which is what removed the second
    /// buffer — 244 MB on a 61 MP channel, and therefore part of how many
    /// frames fit a budget at once.
    pub fn compute(data: &[f32], width: usize, height: usize) -> Self {
        let (count, mean, sigma) = mean_sigma(data);
        let (min, max) = min_max(data).unwrap_or((0.0, 0.0));

        let median = median(data);
        let mad_value = mad(data, median);

        // The clip needs a buffer it may reorder and shrink; the two order
        // statistics above no longer do.
        let mut values = non_null(data);
        let (location, scale) = clipped_location_scale(&mut values, median, mad_value)
            // A frame with no measurable spread — a synthetic flat, a
            // fully saturated region — still needs usable numbers
            // downstream.
            .unwrap_or((median as f64, MAD_TO_SIGMA * mad_value as f64));

        Self {
            count,
            min,
            max,
            mean,
            sigma,
            median,
            mad: mad_value,
            background_noise: background_noise(data, width, height),
            location,
            scale,
        }
    }
}

/// Quickselect: partially orders `data` so that index `k` holds the value it
/// would have if `data` were sorted, and returns it.
fn select_nth(data: &mut [f32], k: usize) -> f32 {
    let (mut lo, mut hi) = (0usize, data.len() - 1);
    loop {
        if lo >= hi {
            return data[k];
        }
        let (lt, gt) = partition_three_way(data, lo, hi);
        if k < lt {
            hi = lt - 1;
        } else if k > gt {
            lo = gt + 1;
        } else {
            // `k` fell inside the run of values equal to the pivot.
            return data[k];
        }
    }
}

/// Three-way partition of `data[lo..=hi]` about a median-of-three pivot,
/// returning `(lt, gt)` such that `data[lo..lt] < pivot`,
/// `data[lt..=gt] == pivot`, and `data[gt+1..=hi] > pivot`.
///
/// The three-way split is not an optimization, it is what makes this
/// terminate on real frames. A calibration frame is millions of pixels spread
/// over a few hundred distinct values, so the pivot is very often the modal
/// value and a two-way partition puts every one of its duplicates on the same
/// side — shrinking the search by a single element per pass and turning the
/// selection quadratic. Collapsing equal values into their own run keeps it
/// linear no matter how few distinct values there are.
fn partition_three_way(data: &mut [f32], lo: usize, hi: usize) -> (usize, usize) {
    let pivot = median_of_three(data, lo, hi);

    // Signed, because `gt` legitimately walks back past `lo` mid-loop.
    let mut lt = lo as isize;
    let mut i = lo as isize;
    let mut gt = hi as isize;

    while i <= gt {
        let v = data[i as usize];
        if v < pivot {
            data.swap(lt as usize, i as usize);
            lt += 1;
            i += 1;
        } else if v > pivot {
            data.swap(i as usize, gt as usize);
            gt -= 1;
        } else {
            i += 1;
        }
    }

    // The pivot was read from this range and never left it, so at least one
    // element equals it and `lt <= gt` holds.
    (lt as usize, gt as usize)
}

/// Orders `data[lo]`, the midpoint, and `data[hi]`, returning the middle one.
///
/// Image rows arrive partially sorted often enough — flat backgrounds,
/// gradients, saturated runs — that a first-element pivot would be a poor
/// choice on real data.
fn median_of_three(data: &mut [f32], lo: usize, hi: usize) -> f32 {
    let mid = lo + (hi - lo) / 2;
    if data[mid] < data[lo] {
        data.swap(mid, lo);
    }
    if data[hi] < data[lo] {
        data.swap(hi, lo);
    }
    if data[hi] < data[mid] {
        data.swap(hi, mid);
    }
    data[mid]
}
