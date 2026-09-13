//! Coarse span timing for the native pipelines (ADR 007, P12-T1).
//!
//! The question this answers is "where does the time actually go", and it is
//! answered by naming a handful of stages in the hot paths and accumulating
//! how long each one takes. That is deliberately not a sampling profiler:
//! a sampler tells you which *instructions* are hot, which is the right tool
//! once a stage is known to be the problem, and the wrong one for deciding
//! which stage that is.
//!
//! Recording is off unless [`enable`] is called, and when it is off a span
//! costs one relaxed atomic load and nothing else — so the instrumentation
//! can live on the paths users run rather than behind a feature flag that
//! rots.
//!
//! Two properties of the numbers matter when reading a report:
//!
//! - **Totals are inclusive.** A span that contains another counts the inner
//!   one's time too, so sibling spans sum to their parent and not to the wall
//!   clock.
//! - **Totals are thread-time.** Spans taken inside a rayon worker accumulate
//!   once per worker, so a parallel stage's total is expected to exceed the
//!   wall clock by roughly the number of cores. A stage whose total is *not*
//!   scaled that way is a stage that is not actually running in parallel,
//!   which is itself a finding.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

static ENABLED: AtomicBool = AtomicBool::new(false);

/// Insertion order is kept alongside the totals so a report can be printed in
/// pipeline order when two stages take a similar amount of time; sorting by
/// total alone makes the table jump around between runs.
type Totals = HashMap<&'static str, Entry>;

fn totals() -> &'static Mutex<Totals> {
    static TOTALS: OnceLock<Mutex<Totals>> = OnceLock::new();
    TOTALS.get_or_init(|| Mutex::new(HashMap::new()))
}

#[derive(Clone, Copy, Default)]
struct Entry {
    nanos: u128,
    calls: u64,
    order: u64,
}

/// Turns recording on. Idempotent.
pub fn enable() {
    ENABLED.store(true, Ordering::Relaxed);
}

/// Turns recording off, leaving whatever has been recorded so far intact.
pub fn disable() {
    ENABLED.store(false, Ordering::Relaxed);
}

pub fn is_enabled() -> bool {
    ENABLED.load(Ordering::Relaxed)
}

/// Discards everything recorded so far.
pub fn reset() {
    if let Ok(mut totals) = totals().lock() {
        totals.clear();
    }
}

/// Times the enclosing scope under `name`, which is a dotted path
/// (`"stack.read"`) so related stages group together in a report.
///
/// The guard must be bound — `let _guard = scope(..)`, not `let _ = scope(..)`,
/// which drops it immediately and times nothing.
#[must_use = "the span ends when the guard is dropped; bind it to a named local"]
pub fn scope(name: &'static str) -> Scope {
    Scope {
        name,
        started: is_enabled().then(Instant::now),
    }
}

/// Records `elapsed` against `name` directly, for a stage whose duration is
/// measured by something other than a scope guard.
pub fn record(name: &'static str, elapsed: Duration) {
    if !is_enabled() {
        return;
    }
    if let Ok(mut totals) = totals().lock() {
        let next = totals.len() as u64;
        let entry = totals.entry(name).or_insert(Entry {
            order: next,
            ..Entry::default()
        });
        entry.nanos += elapsed.as_nanos();
        entry.calls += 1;
    }
}

pub struct Scope {
    name: &'static str,
    started: Option<Instant>,
}

impl Drop for Scope {
    fn drop(&mut self) {
        if let Some(started) = self.started {
            record(self.name, started.elapsed());
        }
    }
}

/// One stage's accumulated time.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sample {
    pub name: &'static str,
    pub calls: u64,
    pub total: Duration,
}

/// Everything recorded so far, in the order the stages were first entered.
pub fn report() -> Vec<Sample> {
    let Ok(totals) = totals().lock() else {
        return Vec::new();
    };
    let mut samples: Vec<(u64, Sample)> = totals
        .iter()
        .map(|(name, entry)| {
            (
                entry.order,
                Sample {
                    name,
                    calls: entry.calls,
                    total: Duration::from_nanos(entry.nanos.min(u64::MAX as u128) as u64),
                },
            )
        })
        .collect();
    samples.sort_by_key(|(order, _)| *order);
    samples.into_iter().map(|(_, sample)| sample).collect()
}

/// Renders [`report`] as a table, against a wall clock for context.
pub fn render(wall: Duration) -> String {
    let samples = report();
    if samples.is_empty() {
        return String::new();
    }
    let width = samples
        .iter()
        .map(|s| s.name.len())
        .max()
        .unwrap_or(4)
        .max(5);
    let wall_seconds = wall.as_secs_f64();

    let mut out = String::new();
    let _ = writeln!(
        out,
        "{:width$}  {:>10}  {:>8}  {:>9}  {:>12}",
        "stage", "total (s)", "x wall", "calls", "mean (ms)"
    );
    for sample in &samples {
        let seconds = sample.total.as_secs_f64();
        let mean_ms = if sample.calls == 0 {
            0.0
        } else {
            seconds * 1000.0 / sample.calls as f64
        };
        let ratio = if wall_seconds > 0.0 {
            seconds / wall_seconds
        } else {
            0.0
        };
        let _ = writeln!(
            out,
            "{:width$}  {:>10.3}  {:>8.2}  {:>9}  {:>12.3}",
            sample.name, seconds, ratio, sample.calls, mean_ms
        );
    }
    let _ = writeln!(out, "\nwall clock: {wall_seconds:.3} s");
    let _ = writeln!(
        out,
        "totals are inclusive and per-thread: nested spans double-count, and a \
         parallel stage\nexceeds the wall clock by roughly its worker count."
    );
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The recorder is process-global, so the tests that touch it run under one
    /// lock rather than as separate `#[test]` functions racing each other.
    #[test]
    fn records_only_while_enabled() {
        static GUARD: Mutex<()> = Mutex::new(());
        let _held = GUARD.lock().unwrap_or_else(|e| e.into_inner());

        reset();
        disable();
        {
            let _span = scope("off");
        }
        assert!(report().is_empty(), "a disabled scope records nothing");

        enable();
        {
            let _span = scope("on");
        }
        {
            let _span = scope("on");
        }
        record("manual", Duration::from_millis(5));
        let samples = report();
        disable();

        assert_eq!(samples.len(), 2, "{samples:?}");
        assert_eq!(samples[0].name, "on");
        assert_eq!(samples[0].calls, 2);
        assert_eq!(samples[1].name, "manual");
        assert_eq!(samples[1].total, Duration::from_millis(5));

        // Rendering is part of the contract: a report with no wall clock must
        // not divide by zero.
        let rendered = render(Duration::ZERO);
        assert!(rendered.contains("manual"), "{rendered}");

        reset();
        assert!(report().is_empty());
    }
}
