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

/// Quotes a path that appears as a bare word in a Siril script.
///
/// Astro filenames routinely contain spaces ("NGC 7000"). Siril's script
/// parser honours a quote only at the *start* of a word, so this works for
/// bare arguments (`cd X`, `load X`, `calibrate_single X`) and not for
/// `-flag=value` forms — see [`flag_value`].
fn quoted(path: &Path) -> String {
    format!("\"{}\"", path.display())
}

/// A path used as a `-flag=value` argument, which Siril cannot parse quoted:
/// the quote would sit after the `=`, where the parser does not look for it.
/// Such a path must therefore contain no whitespace.
fn flag_value(path: &Path) -> Result<String> {
    let text = path.display().to_string();
    if text.contains(char::is_whitespace) {
        bail!(
            "{text} is passed to siril as a -flag=value argument and contains whitespace, \
             which siril's script parser cannot handle. Move the reference output somewhere \
             without spaces in its path."
        );
    }
    Ok(text)
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

        if self.masters || self.calibration {
            stack_masters(sh, &siril, &corpus, &out)?;
        }
        if self.calibration {
            build_calibration_reference(sh, &siril, &corpus, &out)?;
        }
        if self.stars {
            build_star_reference(sh, &siril, &corpus, &out, per_kind)?;
        }
        if self.registration {
            let frames = self.reg_frames.unwrap_or(100) as usize;
            build_registration_reference(sh, &siril, &corpus, &out, frames)?;
        }
        Ok(())
    }
}

/// Median-stacks master bias and dark frames with Siril, issuing exactly the
/// commands `px --engine siril` issues today so the comparison is against what
/// the project actually ships.
fn stack_masters(sh: &Shell, siril: &Path, corpus: &Path, out: &Path) -> Result<()> {
    let masters = out.join("masters");
    std::fs::create_dir_all(&masters)?;

    for kind in ["bias", "dark"] {
        let dir = corpus.join(kind);
        if !dir.is_dir() {
            continue;
        }
        let frames = std::fs::read_dir(&dir)?.count();
        let target = masters.join(format!("master_{kind}"));
        let produced = target.with_extension("fits");
        if produced.exists() {
            println!("  master {kind}: already present, skipping");
            continue;
        }

        // Siril converts into a sequence in its working directory; keep that
        // out of the corpus so the inputs stay pristine.
        let work = sh.create_temp_dir()?;
        let work_path = work.path();
        let script = work_path.join("stack.ssf");
        std::fs::write(
            &script,
            format!(
                "requires 1.2.0\nsetext fits\ncd {}\nconvert {kind}_ -out={}\ncd {}\nstack {kind}_ med -out={}\n",
                quoted(&dir),
                flag_value(work_path)?,
                quoted(work_path),
                flag_value(&target)?
            ),
        )?;

        println!("  stacking master {kind} from {frames} frames (this takes a while)...");
        let started = std::time::Instant::now();
        let log = cmd!(sh, "{siril} -s {script}").ignore_status().read()?;
        let elapsed = started.elapsed();

        if !produced.exists() {
            bail!(
                "siril did not produce {}. Log tail:\n{}",
                produced.display(),
                log.lines().rev().take(15).collect::<Vec<_>>().join("\n")
            );
        }
        println!(
            "  master {kind}: {} in {:.1}s",
            produced.display(),
            elapsed.as_secs_f64()
        );
        std::fs::write(
            masters.join(format!("master_{kind}.log")),
            format!(
                "frames: {frames}\nelapsed_seconds: {:.3}\n\n{log}",
                elapsed.as_secs_f64()
            ),
        )?;
    }
    Ok(())
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

/// Builds a master flat and calibrates one light frame, issuing the same
/// commands `px --engine siril` issues today.
///
/// Two master flats are produced: one Winsorized, which is what the pipeline
/// defaults to, and one linear-fit, which is the method ADR 007 Phase 3
/// diverges from deliberately and therefore needs measuring.
fn build_calibration_reference(sh: &Shell, siril: &Path, corpus: &Path, out: &Path) -> Result<()> {
    let masters = out.join("masters");
    let bias = masters.join("master_bias.fits");
    let dark = masters.join("master_dark.fits");
    if !bias.exists() || !dark.exists() {
        bail!("master bias and dark must exist first (run with --masters)");
    }

    let flat_dir = corpus.join("flat");
    if !flat_dir.is_dir() {
        bail!("no flat directory under {}", corpus.display());
    }

    for (name, rejection) in [("winsorized", "w 3 3"), ("linear", "l 5 5")] {
        let target = masters.join(format!("master_flat_{name}"));
        let produced = target.with_extension("fits");
        if produced.exists() {
            println!("  master flat ({name}): already present, skipping");
            continue;
        }

        let work = sh.create_temp_dir()?;
        let work_path = work.path();
        let script = work_path.join("flat.ssf");
        std::fs::write(
            &script,
            format!(
                "requires 1.2.0\nsetext fits\ncd {}\nconvert flat_ -out={}\ncd {}\n\
                 calibrate flat_ -bias={}\nstack pp_flat_ rej {rejection} -nonorm -out={}\n",
                quoted(&flat_dir),
                flag_value(work_path)?,
                quoted(work_path),
                flag_value(&bias)?,
                flag_value(&target)?
            ),
        )?;

        println!("  building master flat ({name}, rej {rejection})...");
        let started = std::time::Instant::now();
        let log = cmd!(sh, "{siril} -s {script}").ignore_status().read()?;
        if !produced.exists() {
            bail!(
                "siril did not produce {}. Log tail:\n{}",
                produced.display(),
                log.lines().rev().take(20).collect::<Vec<_>>().join("\n")
            );
        }
        println!(
            "  master flat ({name}): {} in {:.1}s",
            produced.display(),
            started.elapsed().as_secs_f64()
        );
        std::fs::write(masters.join(format!("master_flat_{name}.log")), log)?;
    }

    // Calibrate a single light. The master dark is a median of *raw* darks and
    // therefore already carries the bias pedestal, so the light is calibrated
    // with the dark and the (bias-calibrated) flat only. Subtracting a master
    // bias as well would remove that pedestal twice.
    let flat = masters.join("master_flat_winsorized.fits");
    let light = sample_frames(&corpus.join("light"), 1)?
        .into_iter()
        .next()
        .context("no light frames in the corpus")?;

    let calibrated_dir = out.join("calibrated");
    std::fs::create_dir_all(&calibrated_dir)?;
    // The script sets `setext fits`, so Siril writes `pp_<stem>.fits`
    // regardless of whether the input was `.fit` or `.fits`. Built by
    // concatenation rather than `with_extension`, because these filenames
    // carry dots inside them ("300.00s", "14.4F") and `with_extension` would
    // truncate at the last one.
    let emitted_name = format!("pp_{}.fits", light.file_stem().unwrap().to_string_lossy());
    let produced = calibrated_dir.join(&emitted_name);
    if produced.exists() {
        println!("  calibrated light: already present, skipping");
        return Ok(());
    }

    let work = sh.create_temp_dir()?;
    let work_path = work.path();
    let script = work_path.join("cal.ssf");
    std::fs::write(
        &script,
        format!(
            "requires 1.2.0\nsetext fits\ncd {}\ncalibrate_single {} -dark={} -flat={}\n",
            quoted(work_path),
            quoted(&light),
            flag_value(&dark)?,
            flag_value(&flat)?
        ),
    )?;

    println!("  calibrating one light frame...");
    let log = cmd!(sh, "{siril} -s {script}").ignore_status().read()?;

    // `calibrate_single` writes pp_<name> into the working directory.
    let emitted = work_path.join(&emitted_name);
    if !emitted.exists() {
        bail!(
            "siril did not produce {}. Log tail:\n{}",
            emitted.display(),
            log.lines().rev().take(20).collect::<Vec<_>>().join("\n")
        );
    }
    std::fs::copy(&emitted, &produced)?;
    // A second reference with debayering on, so the demosaic can be measured
    // in isolation: the same calibration, the only difference being the CFA
    // interpolation.
    if Frame_is_cfa(&light) {
        let debayer_name = format!(
            "pp_debayered_{}.fits",
            light.file_stem().unwrap().to_string_lossy()
        );
        let debayered = calibrated_dir.join(&debayer_name);
        if !debayered.exists() {
            let work = sh.create_temp_dir()?;
            let work_path = work.path();
            let script = work_path.join("deb.ssf");
            std::fs::write(
                &script,
                format!(
                    "requires 1.2.0\nsetext fits\ncd {}\n\
                     calibrate_single {} -dark={} -flat={} -cfa -debayer\n",
                    quoted(work_path),
                    quoted(&light),
                    flag_value(&dark)?,
                    flag_value(&flat)?
                ),
            )?;
            println!("  calibrating + debayering one light frame...");
            let log = cmd!(sh, "{siril} -s {script}").ignore_status().read()?;
            let emitted = work_path.join(&emitted_name);
            if emitted.exists() {
                std::fs::copy(&emitted, &debayered)?;
                println!("  debayered light: {}", debayered.display());
            } else {
                println!("  (no debayered output; frame is probably not CFA)");
            }
            std::fs::write(calibrated_dir.join("debayer.log"), log)?;
        }
    }

    std::fs::write(
        calibrated_dir.join("source.txt"),
        format!(
            "light: {}\ndark: {}\nflat: {}\nbias: (none - the dark carries it)\n\
             master_bias_used_for_flat: {}\n",
            light.display(),
            dark.display(),
            flat.display(),
            bias.display()
        ),
    )?;
    std::fs::write(calibrated_dir.join("calibrate.log"), log)?;
    println!("  calibrated light: {}", produced.display());
    Ok(())
}

/// Star-detection parameters the reference is generated with.
///
/// Set explicitly rather than left at Siril's defaults, so that a future
/// version changing a default cannot silently move the reference the parity
/// test compares against.
const FINDSTAR_RADIUS: u32 = 5;
const FINDSTAR_SIGMA: &str = "1.0";
const FINDSTAR_ROUNDNESS: &str = "0.5";

/// Runs Siril's `findstar` over a sample of light frames, recording one star
/// list per frame for the ADR 007 P7-T6 parity test.
///
/// The lists are Siril's own tab-separated format, header comments included —
/// the test parses them rather than a re-rendered JSON, so there is one fewer
/// place for a transcription error to hide.
fn build_star_reference(
    sh: &Shell,
    siril: &Path,
    corpus: &Path,
    out: &Path,
    frames: usize,
) -> Result<()> {
    let stars = out.join("stars");
    std::fs::create_dir_all(&stars)?;

    let lights = corpus.join("light");
    if !lights.is_dir() {
        bail!("no light directory under {}", corpus.display());
    }

    let mut manifest = String::new();
    let _ = writeln!(
        manifest,
        "# findstar reference: radius={FINDSTAR_RADIUS} sigma={FINDSTAR_SIGMA} \
roundness={FINDSTAR_ROUNDNESS}"
    );

    for frame in sample_frames(&lights, frames)? {
        let stem = frame.file_stem().unwrap().to_string_lossy().to_string();
        let list = stars.join(format!("{stem}.lst"));
        let relative = frame
            .strip_prefix(corpus)
            .unwrap_or(&frame)
            .display()
            .to_string();
        let _ = writeln!(manifest, "{relative}\t{stem}.lst");

        if list.exists() {
            println!("  stars for {stem}: already present, skipping");
            continue;
        }

        let work = sh.create_temp_dir()?;
        let script = work.path().join("stars.ssf");
        std::fs::write(
            &script,
            format!(
                "requires 1.2.0\nsetext fits\ncd {}\nload {}\n\
                 setfindstar reset -radius={FINDSTAR_RADIUS} -sigma={FINDSTAR_SIGMA} \
                 -roundness={FINDSTAR_ROUNDNESS}\nfindstar -out={}\n",
                quoted(work.path()),
                quoted(&frame),
                flag_value(&list)?
            ),
        )?;

        println!("  finding stars in {stem}...");
        let started = std::time::Instant::now();
        let log = cmd!(sh, "{siril} -s {script}").ignore_status().read()?;
        if !list.exists() {
            bail!(
                "siril did not produce {}. Log tail:\n{}",
                list.display(),
                log.lines().rev().take(15).collect::<Vec<_>>().join("\n")
            );
        }
        let found = log
            .lines()
            .find(|l| l.contains("stars in image"))
            .unwrap_or("(no count line)")
            .trim();
        println!(
            "  {stem}: {found} in {:.2}s",
            started.elapsed().as_secs_f64()
        );
        std::fs::write(stars.join(format!("{stem}.log")), &log)?;
    }

    std::fs::write(stars.join("frames.txt"), manifest)?;
    Ok(())
}

/// Two-pass registers a run of light frames and keeps Siril's sequence file,
/// which carries one homography per frame (ADR 007 P8-T6).
///
/// `register -2pass` computes the transforms without writing registered images,
/// which is exactly what the parity test needs and saves writing a hundred
/// 61 MP frames to disk. Siril's `convert` symlinks FITS input rather than
/// copying it, so the working sequence costs kilobytes.
fn build_registration_reference(
    sh: &Shell,
    siril: &Path,
    corpus: &Path,
    out: &Path,
    frames: usize,
) -> Result<()> {
    let dir = out.join("registration");
    let work = dir.join("work");
    let sequence = work.join("reg_.seq");
    if sequence.exists() {
        println!(
            "  registration: {} already present, skipping",
            sequence.display()
        );
        return Ok(());
    }
    std::fs::create_dir_all(&work)?;

    let lights = corpus.join("light");
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&lights)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| {
            p.extension()
                .and_then(|e| e.to_str())
                .is_some_and(|e| matches!(e.to_ascii_lowercase().as_str(), "fits" | "fit" | "fts"))
        })
        .collect();
    paths.sort();
    paths.truncate(frames);
    if paths.len() < 2 {
        bail!(
            "need at least two light frames to register, found {}",
            paths.len()
        );
    }

    // Siril converts whatever is in one directory, so the frames to register are
    // linked into a directory of their own rather than the whole corpus.
    let staged = work.join("src");
    std::fs::create_dir_all(&staged)?;
    for path in &paths {
        let link = staged.join(path.file_name().unwrap());
        if !link.exists() {
            std::os::unix::fs::symlink(path, &link)?;
        }
    }

    let script = work.join("register.ssf");
    std::fs::write(
        &script,
        format!(
            "requires 1.2.0\nsetext fits\ncd {}\nconvert reg -out={}\ncd {}\nregister reg_ -2pass\n",
            quoted(&staged),
            flag_value(&work)?,
            quoted(&work)
        ),
    )?;

    println!(
        "  two-pass registering {} light frames (this takes a while)...",
        paths.len()
    );
    let started = std::time::Instant::now();
    let log = cmd!(sh, "{siril} -s {script}").ignore_status().read()?;
    if !sequence.exists() {
        bail!(
            "siril did not produce {}. Log tail:\n{}",
            sequence.display(),
            log.lines().rev().take(20).collect::<Vec<_>>().join("\n")
        );
    }
    println!(
        "  registration: {} in {:.1}s",
        sequence.display(),
        started.elapsed().as_secs_f64()
    );
    std::fs::write(dir.join("register.log"), &log)?;

    // The sequence file indexes frames by position; the parity test needs the
    // original paths, so the mapping is recorded alongside it.
    let mut manifest = String::new();
    for (i, path) in paths.iter().enumerate() {
        let relative = path.strip_prefix(corpus).unwrap_or(path).display();
        let _ = writeln!(manifest, "{}\t{relative}", i + 1);
    }
    std::fs::write(dir.join("frames.txt"), manifest)?;
    Ok(())
}

/// Whether a frame declares a Bayer pattern, read straight from its header
/// block. Cheap enough not to warrant pulling a FITS reader into the build
/// tool.
#[allow(non_snake_case)]
fn Frame_is_cfa(path: &Path) -> bool {
    use std::io::Read;
    let Ok(mut file) = std::fs::File::open(path) else {
        return false;
    };
    let mut head = vec![0u8; 2880 * 4];
    if file.read_exact(&mut head).is_err() {
        return false;
    }
    String::from_utf8_lossy(&head).contains("BAYERPAT")
}
