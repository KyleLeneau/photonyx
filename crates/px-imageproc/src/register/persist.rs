//! Registration data as a sidecar FITS table (ADR 007, D6 / P9-T5).
//!
//! The native engine keeps no Siril `.seq` file, but registration still has to
//! survive between steps: `px project stack` registers once and may warp and
//! stack in a later process. So a run's transforms, star counts and exclusions
//! are written next to the output as a FITS binary table — a format the project
//! already reads and writes, that other tools can open, and that carries the
//! `f64` matrix coefficients exactly rather than through a decimal rendering.
//!
//! One table row per input frame, including the excluded ones: "this frame was
//! dropped, and why" is the part a user most needs to see again later.

use std::path::{Path, PathBuf};

use nalgebra::Matrix3;
use px_fits::reader::FitsReader;
use px_fits::table::{BinTableBuilder, Cell};
use px_fits::{BitPix, FitsWriter, HeaderBuilder, card::Value};

use super::framing::OutputGeometry;
use super::psf::Profile;
use super::starfind::StarfindOptions;
use super::transform::Transform;
use super::twopass::{Excluded, FrameRegistration, Registration};
use crate::error::ImageProcError;
use crate::runtime::ImageDims;

/// `EXTNAME` of the table, so a reader can tell this extension from any other
/// one a future version adds to the same file.
const EXTNAME: &str = "PXREGISTRATION";

/// Shortest `PATH` column width, for the degenerate case of a sequence of
/// one-character names.
const MIN_PATH_WIDTH: usize = 8;

/// Writes a registration run to `path`, replacing any existing file.
///
/// The file is a minimal empty primary HDU followed by the table, because a
/// `BINTABLE` is always an extension — a FITS file cannot begin with one.
pub fn write_registration(path: &Path, registration: &Registration) -> Result<(), ImageProcError> {
    let path_width = registration
        .frames
        .iter()
        .map(|f| f.path.as_os_str().len())
        .max()
        .unwrap_or(MIN_PATH_WIDTH)
        .max(MIN_PATH_WIDTH);

    let mut table = BinTableBuilder::new()
        .column("INDEX", "1J")?
        .column("INCLUDED", "1L")?
        .column("EXCLUDED", "16A")?
        .column("STARS", "1J")?
        .column("FWHM", "1E")?
        .column("ROUNDNESS", "1E")?
        .column("INLIERS", "1J")?
        .column("RMSE", "1E")?
        .column("H", "9D")?
        .column("PATH", &format!("{path_width}A"))?;

    table = table
        .card("EXTNAME", Value::String(EXTNAME.to_string()), None)
        .card(
            "REFFRAME",
            Value::Integer(registration.reference as i64),
            Some("0-based index of the reference frame"),
        )
        .card(
            "TRIALS",
            Value::Integer(registration.trials as i64),
            Some("detection trials the loop needed"),
        )
        .card(
            "OUTWIDTH",
            Value::Integer(registration.geometry.dims.width as i64),
            None,
        )
        .card(
            "OUTHEIGH",
            Value::Integer(registration.geometry.dims.height as i64),
            None,
        )
        .card(
            "OUTCHANS",
            Value::Integer(registration.geometry.dims.channels as i64),
            None,
        )
        .card(
            "ROIX",
            Value::Float(registration.geometry.x),
            Some("output origin within the reference frame"),
        )
        .card("ROIY", Value::Float(registration.geometry.y), None);
    table = write_starfind(table, &registration.starfind);

    for frame in &registration.frames {
        // An excluded frame has no transform; the identity is stored so the
        // column has a defined value, and `INCLUDED` is what decides whether it
        // means anything.
        let matrix = frame.transform.unwrap_or_else(Transform::identity).matrix();
        table = table.push_row(vec![
            Cell::Int(frame.index as i64),
            Cell::Bool(frame.included()),
            Cell::Str(exclusion_code(frame.excluded).to_string()),
            Cell::Int(frame.stars as i64),
            Cell::Float(frame.fwhm),
            Cell::Float(frame.roundness),
            Cell::Int(frame.inliers as i64),
            Cell::Float(frame.rmse),
            // Row-major, explicitly: `nalgebra`'s own iterator is
            // column-major, and a transposed matrix here is a silently wrong
            // transform rather than a malformed file.
            Cell::Floats(
                (0..3)
                    .flat_map(|row| (0..3).map(move |column| matrix[(row, column)]))
                    .collect(),
            ),
            Cell::Str(frame.path.to_string_lossy().to_string()),
        ])?;
    }

    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| {
            ImageProcError::Processing(format!("creating {}: {e}", parent.display()))
        })?;
    }
    let mut writer = FitsWriter::create(path)?;
    writer.write_image::<u8>(&HeaderBuilder::primary_image(BitPix::U8, &[])?, &[])?;
    writer.write_bintable(&table)?;
    writer.finish()?;
    Ok(())
}

/// Reads back what [`write_registration`] wrote.
pub fn read_registration(path: &Path) -> Result<Registration, ImageProcError> {
    let reader = FitsReader::open(path)?;
    let table = reader.bintable(1)?;
    let header = table.header();

    let integer = |keyword: &str| -> Result<i64, ImageProcError> {
        header.get_i64(keyword).ok_or_else(|| {
            ImageProcError::Registration(format!(
                "{} is missing {keyword}; is it a registration sidecar?",
                path.display()
            ))
        })
    };
    let reference = integer("REFFRAME")? as usize;
    let geometry = OutputGeometry {
        dims: ImageDims::new(
            integer("OUTWIDTH")? as usize,
            integer("OUTHEIGH")? as usize,
            integer("OUTCHANS")? as usize,
        ),
        x: header.get_f64("ROIX").unwrap_or(0.0),
        y: header.get_f64("ROIY").unwrap_or(0.0),
    };

    let mut frames = Vec::with_capacity(table.nrows());
    for row in 0..table.nrows() {
        let cells = table.row(row)?;
        let int = |i: usize| cells[i].as_i64().unwrap_or(0);
        let float = |i: usize| cells[i].as_f64().unwrap_or(0.0);

        let included = cells[1].as_i64().unwrap_or(0) != 0;
        let coefficients = cells[8].as_f64_vec().unwrap_or_default();
        if coefficients.len() != 9 {
            return Err(ImageProcError::Registration(format!(
                "row {row} of {} has {} matrix coefficients, expected 9",
                path.display(),
                coefficients.len()
            )));
        }
        let transform = included
            .then(|| Transform::from_matrix(Matrix3::from_row_slice(&coefficients)))
            .flatten();

        frames.push(FrameRegistration {
            index: int(0) as usize,
            path: PathBuf::from(cells[9].as_str().unwrap_or_default().trim_end()),
            transform,
            excluded: exclusion_from_code(cells[2].as_str().unwrap_or("").trim_end()),
            stars: int(3) as usize,
            fwhm: float(4),
            roundness: float(5),
            inliers: int(6) as usize,
            rmse: float(7),
        });
    }

    Ok(Registration {
        reference,
        frames,
        geometry,
        trials: integer("TRIALS").unwrap_or(1) as usize,
        starfind: read_starfind(header),
    })
}

/// The star-finder settings a run used, as header cards.
///
/// Stored because a registration cannot be meaningfully re-used or re-trialled
/// without them: "412 stars" means different things at different thresholds.
fn write_starfind(table: BinTableBuilder, options: &StarfindOptions) -> BinTableBuilder {
    table
        .card("SFRADIUS", Value::Integer(options.radius as i64), None)
        .card("SFSIGMA", Value::Float(options.sigma), None)
        .card("SFROUND", Value::Float(options.roundness), None)
        .card("SFMAXR", Value::Float(options.max_roundness), None)
        .card("SFMINA", Value::Float(options.min_amplitude as f64), None)
        .card("SFMAXA", Value::Float(options.max_amplitude as f64), None)
        .card("SFCONV", Value::Integer(options.convergence as i64), None)
        .card("SFRELAX", Value::Logical(options.relax_checks), None)
        .card("SFMAXSTR", Value::Integer(options.max_stars as i64), None)
        .card(
            "SFPROF",
            Value::String(
                match options.profile {
                    Profile::Gaussian => "gaussian",
                    Profile::Moffat => "moffat",
                }
                .to_string(),
            ),
            None,
        )
        .card("SFMINBET", Value::Float(options.min_beta), None)
}

fn read_starfind(header: &px_fits::header::Header) -> StarfindOptions {
    let defaults = StarfindOptions::default();
    StarfindOptions {
        radius: header
            .get_i64("SFRADIUS")
            .map(|v| v as usize)
            .unwrap_or(defaults.radius),
        sigma: header.get_f64("SFSIGMA").unwrap_or(defaults.sigma),
        roundness: header.get_f64("SFROUND").unwrap_or(defaults.roundness),
        max_roundness: header.get_f64("SFMAXR").unwrap_or(defaults.max_roundness),
        min_amplitude: header
            .get_f64("SFMINA")
            .map(|v| v as f32)
            .unwrap_or(defaults.min_amplitude),
        max_amplitude: header
            .get_f64("SFMAXA")
            .map(|v| v as f32)
            .unwrap_or(defaults.max_amplitude),
        convergence: header
            .get_i64("SFCONV")
            .map(|v| v as usize)
            .unwrap_or(defaults.convergence),
        relax_checks: header.get_bool("SFRELAX").unwrap_or(defaults.relax_checks),
        max_stars: header
            .get_i64("SFMAXSTR")
            .map(|v| v as usize)
            .unwrap_or(defaults.max_stars),
        profile: match header.get_string("SFPROF").as_deref().map(str::trim) {
            Some("moffat") => Profile::Moffat,
            _ => Profile::Gaussian,
        },
        min_beta: header.get_f64("SFMINBET").unwrap_or(defaults.min_beta),
    }
}

/// Exclusion reasons round-trip as short codes rather than as the display
/// strings, so the file stays readable if the wording changes.
fn exclusion_code(excluded: Option<Excluded>) -> &'static str {
    match excluded {
        None => "",
        Some(Excluded::NoStars) => "NOSTARS",
        Some(Excluded::TooFewStars) => "FEWSTARS",
        Some(Excluded::MatchFailed) => "NOMATCH",
        Some(Excluded::Unreadable) => "UNREAD",
    }
}

fn exclusion_from_code(code: &str) -> Option<Excluded> {
    match code {
        "NOSTARS" => Some(Excluded::NoStars),
        "FEWSTARS" => Some(Excluded::TooFewStars),
        "NOMATCH" => Some(Excluded::MatchFailed),
        "UNREAD" => Some(Excluded::Unreadable),
        _ => None,
    }
}
