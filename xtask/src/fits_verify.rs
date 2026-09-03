//! Runs external FITS standard-compliance validators (`fitsverify` and/or astropy)
//! over a directory of FITS files, per ADR 006 (O4). Advisory when the tools are
//! not installed — this must never fail CI solely because a machine lacks them.

use std::path::PathBuf;

use anyhow::Result;
use xshell::{Shell, cmd};

use crate::{flags, project_root};

impl flags::FitsVerify {
    pub(crate) fn run(&self, sh: &Shell) -> Result<()> {
        let root = project_root();
        let dir = self
            .dir
            .clone()
            .map(PathBuf::from)
            .unwrap_or_else(|| root.join("crates/px-fits/tests/fixtures"));

        if !dir.exists() {
            anyhow::bail!("directory does not exist: {:?}", dir);
        }

        let files: Vec<PathBuf> = walk_fits(&dir)?;
        if files.is_empty() {
            println!("no .fits/.fit files found under {:?}", dir);
            return Ok(());
        }

        let have_fitsverify = cmd!(sh, "fitsverify -h").ignore_status().output().is_ok();
        let have_python_astropy = cmd!(sh, "python3 -c \"import astropy\"")
            .ignore_status()
            .output()
            .map(|o| o.status.success())
            .unwrap_or(false);

        if !have_fitsverify && !have_python_astropy {
            println!(
                "neither `fitsverify` nor a python3+astropy environment was found; skipping.\n\
                 Install one to validate FITS standard compliance:\n  \
                 https://heasarc.gsfc.nasa.gov/docs/software/ftools/fitsverify/\n  \
                 pip install astropy"
            );
            return Ok(());
        }

        let mut failures = Vec::new();

        if have_fitsverify {
            println!("running fitsverify over {} file(s)...", files.len());
            for file in &files {
                let out = cmd!(sh, "fitsverify -q {file}").ignore_status().output()?;
                if !out.status.success() {
                    failures.push(format!("{file:?}: fitsverify exit {:?}", out.status.code()));
                }
            }
        }

        if have_python_astropy {
            println!(
                "running astropy open-and-verify over {} file(s)...",
                files.len()
            );
            for file in &files {
                let script = format!(
                    "from astropy.io import fits\n\
                     hdul = fits.open(r'{}')\n\
                     hdul.verify('exception')\n\
                     [h.data for h in hdul if h.data is not None]\n",
                    file.display()
                );
                let out = cmd!(sh, "python3 -c {script}").ignore_status().output()?;
                if !out.status.success() {
                    failures.push(format!(
                        "{file:?}: astropy verify failed: {}",
                        String::from_utf8_lossy(&out.stderr)
                    ));
                }
            }
        }

        if failures.is_empty() {
            println!("all {} file(s) passed.", files.len());
            Ok(())
        } else {
            for f in &failures {
                eprintln!("FAIL: {f}");
            }
            anyhow::bail!(
                "{} of {} file(s) failed verification",
                failures.len(),
                files.len()
            );
        }
    }
}

fn walk_fits(dir: &PathBuf) -> Result<Vec<PathBuf>> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(dir)? {
        let entry = entry?;
        let path = entry.path();
        if path.is_dir() {
            out.extend(walk_fits(&path)?);
        } else if matches!(
            path.extension().and_then(|e| e.to_str()),
            Some("fits") | Some("fit")
        ) {
            out.push(path);
        }
    }
    out.sort();
    Ok(out)
}
