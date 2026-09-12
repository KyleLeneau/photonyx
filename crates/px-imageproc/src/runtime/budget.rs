//! Resolving how much memory and how many cores a run may use (ADR 007,
//! P1-T3).
//!
//! A ratio is resolved against *available* memory rather than total: the
//! total is not a number this process can actually use, since the rest of the
//! system already holds some of it. Under a container the cgroup ceiling wins,
//! because inside one the host figure is not what this process may allocate
//! either.

use std::fmt;

/// How the memory ceiling for a run is expressed.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum MemoryLimit {
    /// A fraction of available memory, e.g. `0.9`.
    Ratio(f64),
    /// A fixed number of gibibytes.
    FixedGb(f64),
    /// An exact byte count. Mostly for tests.
    Bytes(u64),
}

impl Default for MemoryLimit {
    fn default() -> Self {
        // Nine tenths: enough headroom for the allocator and the OS without
        // leaving a meaningful share of the machine unused.
        Self::Ratio(0.9)
    }
}

/// A resolved budget: concrete numbers, no further negotiation.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ResourceBudget {
    /// Total bytes the run may hold live at once.
    pub max_bytes: u64,
    /// Worker threads the run may use. Always ≥ 1.
    pub cores: usize,
}

impl ResourceBudget {
    pub fn new(max_bytes: u64, cores: usize) -> Self {
        Self {
            max_bytes,
            cores: cores.max(1),
        }
    }

    /// Resolves a limit against a memory probe.
    ///
    /// `available_bytes` is what the probe reports; see [`available_memory`].
    pub fn resolve(limit: MemoryLimit, cores: Option<usize>, available_bytes: u64) -> Self {
        let max_bytes = match limit {
            MemoryLimit::Ratio(r) => (available_bytes as f64 * r.clamp(0.0, 1.0)) as u64,
            MemoryLimit::FixedGb(gb) => (gb.max(0.0) * 1024.0 * 1024.0 * 1024.0) as u64,
            MemoryLimit::Bytes(b) => b,
        };
        let cores = cores.unwrap_or_else(default_cores).max(1);
        Self { max_bytes, cores }
    }

    /// How many whole frames of `frame_bytes` may be in flight at once, for
    /// the stages that work a frame at a time (statistics, star detection,
    /// warping) rather than in blocks.
    ///
    /// Never returns 0: a budget that cannot hold a single frame still has to
    /// attempt one, and the caller will get an allocation failure that names
    /// the real problem rather than a silent no-op.
    pub fn max_frames_in_flight(&self, frame_bytes: u64) -> usize {
        if frame_bytes == 0 {
            return self.cores;
        }
        ((self.max_bytes / frame_bytes) as usize).clamp(1, self.cores)
    }
}

impl fmt::Display for ResourceBudget {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        const KIB: f64 = 1024.0;
        const MIB: f64 = 1024.0 * KIB;
        const GIB: f64 = 1024.0 * MIB;
        let bytes = self.max_bytes as f64;
        // A budget reported as "0.00 GiB" tells the reader nothing about why
        // their run failed.
        if bytes >= GIB {
            write!(f, "{:.2} GiB", bytes / GIB)?;
        } else if bytes >= MIB {
            write!(f, "{:.1} MiB", bytes / MIB)?;
        } else {
            write!(f, "{:.0} KiB", bytes / KIB)?;
        }
        write!(f, " across {} core(s)", self.cores)
    }
}

/// Cores to use when the caller does not say. Mirrors what a `rayon` default
/// pool would pick.
pub fn default_cores() -> usize {
    std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(1)
}

/// Available system memory in bytes, honouring a cgroup limit when one is set.
///
/// Container limits win over the host figure — inside a 2 GiB container,
/// `sysinfo` still reports the host's free memory, and trusting it is how a
/// process gets OOM-killed.
pub fn available_memory() -> u64 {
    let host = host_available_memory();
    match cgroup_memory_limit() {
        Some(limit) => host.min(limit),
        None => host,
    }
}

fn host_available_memory() -> u64 {
    let mut sys = sysinfo::System::new();
    sys.refresh_memory();
    sys.available_memory()
}

/// Reads a cgroup v1 or v2 memory ceiling, if this process is under one.
///
/// Both cgroup generations are checked, v1 before v2, since a host may expose
/// either. The paths are the documented kernel interface.
fn cgroup_memory_limit() -> Option<u64> {
    const PATHS: [&str; 7] = [
        "/sys/fs/cgroup/memory/memory.limit_in_bytes",
        "/sys/fs/cgroup/memory/memory.low",
        "/sys/fs/cgroup/memory/memory.high",
        "/sys/fs/cgroup/memory/memory.max",
        "/sys/fs/cgroup/memory.low",
        "/sys/fs/cgroup/memory.high",
        "/sys/fs/cgroup/memory.max",
    ];

    for path in PATHS {
        // cgroup v2 writes the literal "max" for "no limit"; v1 writes a
        // sentinel near u64::MAX. Both fail this parse-and-sanity check, which
        // is what we want.
        if let Ok(contents) = std::fs::read_to_string(path)
            && let Ok(bytes) = contents.trim().parse::<u64>()
            && bytes > 0
            && bytes < u64::MAX / 2
        {
            return Some(bytes);
        }
    }
    None
}

/// Cores allowed by a cgroup CPU quota, if one is set.
pub fn cgroup_cpu_limit() -> Option<usize> {
    // cgroup v1: quota / period, with -1 meaning unlimited.
    let quota = std::fs::read_to_string("/sys/fs/cgroup/cpu/cpu.cfs_quota_us")
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok());
    let period = std::fs::read_to_string("/sys/fs/cgroup/cpu/cpu.cfs_period_us")
        .ok()
        .and_then(|s| s.trim().parse::<i64>().ok());
    if let (Some(q), Some(p)) = (quota, period)
        && q > 0
        && p > 0
    {
        return Some(((q / p) as usize).max(1));
    }

    // cgroup v2: "<quota> <period>", with "max" meaning unlimited.
    if let Ok(contents) = std::fs::read_to_string("/sys/fs/cgroup/cpu.max") {
        let mut parts = contents.split_whitespace();
        if let (Some(q), Some(p)) = (parts.next(), parts.next())
            && let (Ok(q), Ok(p)) = (q.parse::<u64>(), p.parse::<u64>())
            && p > 0
        {
            return Some(((q / p) as usize).max(1));
        }
    }

    None
}
