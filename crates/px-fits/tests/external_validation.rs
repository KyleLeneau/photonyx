//! ADR 006 P5-T6 (O4): every file `FitsWriter` produces must pass an
//! external FITS standard-compliance validator.
//!
//! Validator resolution, in order: `fitsverify` on `PATH`; a system
//! `python3` that can `import astropy`; or `uv run --with astropy` (uv
//! provisions astropy on demand). If none is available the test prints a
//! skip notice and passes — O4 is advisory when the tools are absent, but a
//! machine that has any of them (CI, and this dev box via `uv`) enforces it.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use px_fits::header::BitPix;
use px_fits::table::{AsciiTableBuilder, BinTableBuilder};
use px_fits::{Cell, FitsWriter, HeaderBuilder};

fn out_dir() -> PathBuf {
    let d = std::env::temp_dir().join("px-fits-external-validation");
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write<T: px_fits::Pixel>(dir: &Path, name: &str, h: &HeaderBuilder, data: &[T]) -> PathBuf {
    let path = dir.join(name);
    let mut w = FitsWriter::create(&path).unwrap();
    w.write_image(h, data).unwrap();
    w.finish().unwrap();
    path
}

/// Emits a spread of writer-produced files covering the Phase 5 surface.
fn write_sample_files(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();

    files.push(write(
        dir,
        "u8_2d.fits",
        &HeaderBuilder::primary_image(BitPix::U8, &[16, 10]).unwrap(),
        &(0..160).map(|i| i as u8).collect::<Vec<_>>(),
    ));
    files.push(write(
        dir,
        "i16_1d.fits",
        &HeaderBuilder::primary_image(BitPix::I16, &[64]).unwrap(),
        &(-32..32).collect::<Vec<i16>>(),
    ));
    files.push(write(
        dir,
        "i32_3d.fits",
        &HeaderBuilder::primary_image(BitPix::I32, &[4, 3, 2]).unwrap(),
        &(0..24).collect::<Vec<i32>>(),
    ));
    files.push(write(
        dir,
        "i64_2d.fits",
        &HeaderBuilder::primary_image(BitPix::I64, &[5, 5]).unwrap(),
        &(0..25)
            .map(|i| i as i64 * 1_000_000_000)
            .collect::<Vec<_>>(),
    ));
    files.push(write(
        dir,
        "f32_2d.fits",
        &HeaderBuilder::primary_image(BitPix::F32, &[8, 8]).unwrap(),
        &(0..64).map(|i| i as f32 * 0.25 - 3.0).collect::<Vec<_>>(),
    ));
    files.push(write(
        dir,
        "f64_2d.fits",
        &HeaderBuilder::primary_image(BitPix::F64, &[6, 6]).unwrap(),
        &(0..36).map(|i| (i as f64).sqrt()).collect::<Vec<_>>(),
    ));

    // Scaling conventions.
    files.push(write(
        dir,
        "u16_via_bzero.fits",
        &HeaderBuilder::primary_image(BitPix::I16, &[32, 32])
            .unwrap()
            .set_f64("BZERO", 32768.0, Some("unsigned 16-bit"))
            .unwrap()
            .set_f64("BSCALE", 1.0, None)
            .unwrap(),
        &(0..1024).map(|i| (i - 32768) as i16).collect::<Vec<_>>(),
    ));
    files.push(write(
        dir,
        "real_scaling.fits",
        &HeaderBuilder::primary_image(BitPix::I32, &[10, 10])
            .unwrap()
            .set_f64("BSCALE", 0.5, None)
            .unwrap()
            .set_f64("BZERO", 100.0, None)
            .unwrap(),
        &(0..100).collect::<Vec<i32>>(),
    ));
    files.push(write(
        dir,
        "blank.fits",
        &HeaderBuilder::primary_image(BitPix::I16, &[10, 10])
            .unwrap()
            .set_i64("BLANK", -32768, Some("undefined"))
            .unwrap(),
        &(0..100)
            .map(|i| if i % 7 == 0 { -32768 } else { i as i16 })
            .collect::<Vec<_>>(),
    ));

    // Extra user cards (strings, floats, comments, history).
    files.push(write(
        dir,
        "rich_header.fits",
        &HeaderBuilder::primary_image(BitPix::U8, &[4, 4])
            .unwrap()
            .set_str("OBJECT", "M42", Some("target"))
            .unwrap()
            .set_str("FILTER", "Ha", Some("narrowband 3nm"))
            .unwrap()
            .set_f64("EXPTIME", 300.0, Some("seconds"))
            .unwrap()
            .set_i64("GAIN", 100, None)
            .unwrap()
            .comment("written by px-fits FitsWriter")
            .unwrap(),
        &[0u8; 16],
    ));

    // Multi-HDU: data-less primary + IMAGE extension, written via begin_image.
    {
        let path = dir.join("multi_hdu_streamed.fits");
        let mut w = FitsWriter::create(&path).unwrap();
        w.write_image::<u8>(&HeaderBuilder::primary_image(BitPix::U8, &[]).unwrap(), &[])
            .unwrap();
        let ext = HeaderBuilder::image_ext(BitPix::I16, &[20, 15])
            .unwrap()
            .set_str("EXTNAME", "SCI", None)
            .unwrap();
        let mut iw = w.begin_image::<i16>(&ext).unwrap();
        for r in 0..15 {
            let row: Vec<i16> = (0..20).map(|c| (r * 20 + c) as i16).collect();
            iw.write_row(&row).unwrap();
        }
        iw.finish().unwrap();
        w.finish().unwrap();
        files.push(path);
    }

    // BINTABLE with scalar/vector/string/scaled + a variable-length column.
    {
        let path = dir.join("bintable.fits");
        let mut w = FitsWriter::create(&path).unwrap();
        w.write_image::<u8>(&HeaderBuilder::primary_image(BitPix::U8, &[]).unwrap(), &[])
            .unwrap();
        let mut t = BinTableBuilder::new()
            .column("ID", "J")
            .unwrap()
            .column_full("FLUX", "E", Some("Jy"), None, None, None)
            .unwrap()
            .column("NAME", "8A")
            .unwrap()
            .column("COORD", "2D")
            .unwrap()
            .column_full("CNT", "I", None, Some(1.0), Some(32768.0), None)
            .unwrap()
            .column("SAMPLES", "1PJ(4)")
            .unwrap();
        let vla: [&[i64]; 4] = [&[1, 2], &[3], &[], &[4, 5, 6]];
        for r in 0..4i64 {
            t = t
                .push_row(vec![
                    Cell::Int(r + 1),
                    Cell::Float(1.5 * (r + 1) as f64),
                    Cell::Str(format!("obj{r}")),
                    Cell::Floats(vec![r as f64, -(r as f64)]),
                    Cell::Int(30000 + r * 10000),
                    Cell::Ints(vla[r as usize].to_vec()),
                ])
                .unwrap();
        }
        w.write_bintable(&t).unwrap();
        w.finish().unwrap();
        files.push(path);
    }

    // ASCII TABLE with a null cell.
    {
        let path = dir.join("ascii_table.fits");
        let mut w = FitsWriter::create(&path).unwrap();
        w.write_image::<u8>(&HeaderBuilder::primary_image(BitPix::U8, &[]).unwrap(), &[])
            .unwrap();
        let t = AsciiTableBuilder::new()
            .column("SEQ", "I5")
            .unwrap()
            .column_with_unit("MAG", "F8.3", "mag")
            .unwrap()
            .column("LABEL", "A10")
            .unwrap()
            .push_row(vec![
                Cell::Int(1),
                Cell::Float(1.234),
                Cell::Str("hydrogen".into()),
            ])
            .unwrap()
            .push_row(vec![Cell::Int(2), Cell::Null, Cell::Str("helium".into())])
            .unwrap();
        w.write_ascii_table(&t).unwrap();
        w.finish().unwrap();
        files.push(path);
    }

    files
}

enum Validator {
    Fitsverify,
    Python3,
    Uv,
}

impl Validator {
    fn detect() -> Option<Self> {
        if Command::new("fitsverify").arg("-h").output().is_ok() {
            return Some(Self::Fitsverify);
        }
        if run_ok(Command::new("python3").args(["-c", "import astropy"])) {
            return Some(Self::Python3);
        }
        if run_ok(Command::new("uv").arg("--version")) {
            return Some(Self::Uv);
        }
        None
    }

    fn name(&self) -> &'static str {
        match self {
            Self::Fitsverify => "fitsverify",
            Self::Python3 => "system python3 + astropy",
            Self::Uv => "uv run --with astropy",
        }
    }

    fn check(&self, file: &Path) -> Result<(), String> {
        let out: Output = match self {
            Self::Fitsverify => Command::new("fitsverify")
                .arg("-q")
                .arg(file)
                .output()
                .map_err(|e| e.to_string())?,
            Self::Python3 | Self::Uv => {
                let script = format!(
                    "from astropy.io import fits\n\
                     h = fits.open(r'{}')\n\
                     h.verify('exception')\n\
                     [x.data for x in h if x.data is not None]\n",
                    file.display()
                );
                let mut cmd = match self {
                    Self::Uv => {
                        let mut c = Command::new("uv");
                        c.args(["run", "--quiet", "--with", "astropy", "--", "python", "-c"]);
                        c.arg(&script);
                        c
                    }
                    _ => {
                        let mut c = Command::new("python3");
                        c.arg("-c").arg(&script);
                        c
                    }
                };
                cmd.output().map_err(|e| e.to_string())?
            }
        };
        if out.status.success() {
            Ok(())
        } else {
            Err(format!(
                "exit {:?}\nstdout: {}\nstderr: {}",
                out.status.code(),
                String::from_utf8_lossy(&out.stdout),
                String::from_utf8_lossy(&out.stderr)
            ))
        }
    }
}

fn run_ok(cmd: &mut Command) -> bool {
    cmd.output().map(|o| o.status.success()).unwrap_or(false)
}

#[test]
fn every_writer_output_passes_an_external_validator() {
    let dir = out_dir();
    let files = write_sample_files(&dir);
    assert!(files.len() >= 10);

    let Some(validator) = Validator::detect() else {
        eprintln!(
            "SKIP: no FITS validator available (fitsverify / python3+astropy / uv). \
             {} writer-output files left in {} for manual checking.",
            files.len(),
            dir.display()
        );
        return;
    };

    eprintln!(
        "validating {} writer-output files with {}",
        files.len(),
        validator.name()
    );
    let mut failures = Vec::new();
    for f in &files {
        if let Err(e) = validator.check(f) {
            failures.push(format!("{}: {e}", f.file_name().unwrap().to_string_lossy()));
        }
    }
    assert!(
        failures.is_empty(),
        "validator rejected writer output:\n{}",
        failures.join("\n\n")
    );
}
