//! Generates reference outputs from a local Siril install, for the parity
//! tests of ADR 007.
//!
//! Nothing here is committed: the outputs are large and specific to a Siril
//! version, so they live under `data/reference/` (git-ignored) and the tests
//! that consume them skip cleanly when they are absent.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result, bail};
use xshell::{Shell, cmd};

use crate::flags;

/// Where Siril lives, per platform. Mirrors the search `siril-sys` does at
/// runtime; duplicated because that crate is a library and this is a build
/// tool, and neither should depend on the other.
fn siril_binary() -> Option<PathBuf> {
    let candidates: &[&str] = match std::env::consts::OS {
        "macos" => &[
            "/Applications/Siril.app/Contents/MacOS/siril-cli",
            "/opt/homebrew/bin/siril-cli",
            "/usr/local/bin/siril-cli",
        ],
        "windows" => &[
            r"C:\msys64\mingw64\bin\siril-cli.exe",
            r"C:\Program Files\SiriL\bin\siril-cli.exe",
        ],
        _ => &["/usr/local/bin/siril-cli", "/usr/bin/siril-cli"],
    };
    candidates
        .iter()
        .map(PathBuf::from)
        .find(|p| p.exists())
        .or_else(|| which("siril-cli"))
}

fn which(name: &str) -> Option<PathBuf> {
    let path = std::env::var_os("PATH")?;
    std::env::split_paths(&path)
        .map(|dir| dir.join(name))
        .find(|p| p.is_file())
}

/// The statistics Siril's `stat main` reports, in 16-bit ADU.
#[derive(Debug, Clone)]
struct SirilStats {
    layer: String,
    mean: f64,
    median: f64,
    sigma: f64,
    min: f64,
    max: f64,
    bgnoise: f64,
    avg_dev: f64,
    mad: f64,
    sqrt_bwmv: f64,
}

impl flags::SirilReference {
    pub(crate) fn run(self, sh: &Shell) -> Result<()> {
        let root = crate::project_root();
        let corpus = root.join(self.corpus.as_deref().unwrap_or("data/corpus-mono"));
        let out = root.join(self.out.as_deref().unwrap_or("data/reference"));
        let per_kind = self.frames.unwrap_or(3) as usize;

        if !corpus.is_dir() {
            bail!("corpus directory {} does not exist", corpus.display());
        }
        let Some(siril) = siril_binary() else {
            bail!(
                "siril-cli not found. Install Siril, or pass its directory on PATH. \
                 Reference generation is optional — the parity tests skip without it."
            );
        };
        let version = siril_version(sh, &siril)?;
        println!("using siril {version} at {}", siril.display());

        std::fs::create_dir_all(out.join("stats"))?;

        let mut records = Vec::new();
        for kind in ["bias", "dark", "flat", "light"] {
            let dir = corpus.join(kind);
            if !dir.is_dir() {
                println!("  {kind}: no directory, skipping");
                continue;
            }
            for path in sample_frames(&dir, per_kind)? {
                let stats = stat_frame(sh, &siril, &path)?;
                let relative = path
                    .strip_prefix(&corpus)
                    .unwrap_or(&path)
                    .display()
                    .to_string();
                println!(
                    "  {relative}: median {:.1}, sigma {:.1}, bgnoise {:.1}",
                    stats.median, stats.sigma, stats.bgnoise
                );
                records.push((kind.to_string(), relative, stats));
            }
        }

        if records.is_empty() {
            bail!("no frames found under {}", corpus.display());
        }

        let path = out.join("stats").join("stats.json");
        std::fs::write(&path, render_json(&version, &corpus, &records))
            .with_context(|| format!("writing {}", path.display()))?;
        println!("wrote {} records to {}", records.len(), path.display());
        Ok(())
    }
}

fn siril_version(sh: &Shell, siril: &Path) -> Result<String> {
    let output = cmd!(sh, "{siril} --version").ignore_status().read()?;
    Ok(output
        .lines()
        .find_map(|l| l.trim().strip_prefix("siril "))
        .unwrap_or("unknown")
        .trim()
        .to_string())
}

/// Picks up to `count` frames from `dir`, evenly spaced through the sorted
/// listing so the sample spans the session rather than clustering at its start.
fn sample_frames(dir: &Path, count: usize) -> Result<Vec<PathBuf>> {
    let mut all: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "fits" | "fit" | "fts"))
        })
        .collect();
    all.sort();

    if all.len() <= count {
        return Ok(all);
    }
    let step = all.len() / count;
    Ok((0..count).map(|i| all[i * step].clone()).collect())
}

fn stat_frame(sh: &Shell, siril: &Path, frame: &Path) -> Result<SirilStats> {
    let script = sh.create_temp_dir()?;
    let script_path = script.path().join("stat.ssf");
    std::fs::write(
        &script_path,
        format!("requires 1.2.0\nload {}\nstat main\n", frame.display()),
    )?;

    let output = cmd!(sh, "{siril} -s {script_path}")
        .ignore_status()
        .read()
        .with_context(|| format!("running siril stat on {}", frame.display()))?;

    parse_stat(&output)
        .with_context(|| format!("no stat line in siril output for {}", frame.display()))
}

/// Pulls the numbers out of Siril's `stat main` log line, which looks like:
///
/// ```text
/// log: B&W layer: Mean: 301.9, Median: 302.0, Sigma: 6.2, Min: 80.0, Max: 525.0, \
///      bgnoise: 5.2, avgDev: 3.8, MAD: 3.0, sqrt(BWMV): 4.3
/// ```
fn parse_stat(output: &str) -> Option<SirilStats> {
    let line = output.lines().find(|l| l.contains(" layer: Mean:"))?;
    let layer = line
        .split(" layer:")
        .next()?
        .rsplit("log: ")
        .next()?
        .trim()
        .to_string();

    let field = |name: &str| -> Option<f64> {
        let at = line.find(&format!("{name}: "))? + name.len() + 2;
        let rest = &line[at..];
        let end = rest.find(',').unwrap_or(rest.len());
        rest[..end].trim().parse().ok()
    };

    Some(SirilStats {
        layer,
        mean: field("Mean")?,
        median: field("Median")?,
        sigma: field("Sigma")?,
        min: field("Min")?,
        max: field("Max")?,
        bgnoise: field("bgnoise")?,
        avg_dev: field("avgDev")?,
        mad: field("MAD")?,
        sqrt_bwmv: field("sqrt(BWMV)")?,
    })
}

/// Hand-rolled rather than pulling `serde` into the build tool for one
/// flat record type.
fn render_json(version: &str, corpus: &Path, records: &[(String, String, SirilStats)]) -> String {
    let mut s = String::new();
    s.push_str("{\n");
    let _ = writeln!(s, "  \"siril_version\": \"{version}\",");
    let _ = writeln!(s, "  \"corpus\": \"{}\",", corpus.display());
    s.push_str("  \"note\": \"stat main output, 16-bit ADU, printed by Siril at one decimal\",\n");
    s.push_str("  \"frames\": [\n");
    for (i, (kind, path, st)) in records.iter().enumerate() {
        let comma = if i + 1 == records.len() { "" } else { "," };
        let _ = writeln!(
            s,
            "    {{\"kind\": \"{kind}\", \"path\": \"{path}\", \"layer\": \"{}\", \
             \"mean\": {}, \"median\": {}, \"sigma\": {}, \"min\": {}, \"max\": {}, \
             \"bgnoise\": {}, \"avg_dev\": {}, \"mad\": {}, \"sqrt_bwmv\": {}}}{comma}",
            st.layer,
            st.mean,
            st.median,
            st.sigma,
            st.min,
            st.max,
            st.bgnoise,
            st.avg_dev,
            st.mad,
            st.sqrt_bwmv
        );
    }
    s.push_str("  ]\n}\n");
    s
}
