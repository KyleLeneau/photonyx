//! Bayer color filter array pattern detection (ADR 006, Phase 9, P9-T2).
//!
//! `BAYERPAT` is the de-facto standard keyword (SBIG, MaxIm DL, N.I.N.A.,
//! Siril, ...) naming the 2x2 CFA tile starting at the pixel closest to the
//! origin. `XBAYROFF`/`YBAYROFF` shift that origin when the sensor's active
//! area doesn't start on a tile boundary; most consumer astro cameras report
//! 0/0 and omit the keywords entirely, so an absent offset is treated as 0.

use px_fits::header::Header;

/// The 2x2 Bayer CFA tile, named by its top-left-to-bottom-right pixel order
/// (matches the `BAYERPAT` FITS keyword convention).
#[derive(Copy, Clone, PartialEq, Eq, Debug, Default)]
pub enum BayerPattern {
    #[default]
    None,
    Rggb,
    Bggr,
    Gbrg,
    Grbg,
}

impl BayerPattern {
    /// Rotates the pattern by an odd pixel offset in one axis: shifting the
    /// CFA tile by one pixel swaps R and B rows or columns depending on
    /// axis, which is equivalent to swapping R<->G's partner and G<->B's
    /// partner along that axis. Bayer tiles are 2-periodic, so only the
    /// parity of the offset matters.
    fn shifted(self, x_odd: bool, y_odd: bool) -> Self {
        use BayerPattern::*;
        match (x_odd, y_odd) {
            (false, false) => self,
            (true, false) => match self {
                Rggb => Grbg,
                Grbg => Rggb,
                Bggr => Gbrg,
                Gbrg => Bggr,
                None => None,
            },
            (false, true) => match self {
                Rggb => Gbrg,
                Gbrg => Rggb,
                Bggr => Grbg,
                Grbg => Bggr,
                None => None,
            },
            (true, true) => match self {
                Rggb => Bggr,
                Bggr => Rggb,
                Gbrg => Grbg,
                Grbg => Gbrg,
                None => None,
            },
        }
    }
}

/// Reads `BAYERPAT` (and, if present, `XBAYROFF`/`YBAYROFF`) from a primary
/// or image-extension header. Any value other than the four recognized
/// tiles — including an absent keyword, an empty string, or "NONE" — is
/// [`BayerPattern::None`], matching `px_fits::FitsFile::is_color`'s existing
/// `BAYERPAT` handling.
pub fn detect(header: &Header) -> BayerPattern {
    let base = match header.get_string("BAYERPAT").as_deref() {
        Some("RGGB") => BayerPattern::Rggb,
        Some("BGGR") => BayerPattern::Bggr,
        Some("GBRG") => BayerPattern::Gbrg,
        Some("GRBG") => BayerPattern::Grbg,
        _ => return BayerPattern::None,
    };

    let x_odd = header.get_i64("XBAYROFF").unwrap_or(0) % 2 != 0;
    let y_odd = header.get_i64("YBAYROFF").unwrap_or(0) % 2 != 0;
    base.shifted(x_odd, y_odd)
}

#[cfg(test)]
mod tests {
    use super::*;
    use px_fits::card::Value;
    use px_fits::reader::FitsReader;
    use px_fits::{BitPix, FitsWriter, HeaderBuilder};
    use std::path::PathBuf;

    fn tmp(name: &str) -> PathBuf {
        let dir = std::env::temp_dir().join("px-imageproc-bayer-tests");
        std::fs::create_dir_all(&dir).unwrap();
        dir.join(name)
    }

    fn write_header(name: &str, cards: &[(&str, Value)]) -> PathBuf {
        let path = tmp(name);
        let mut hb = HeaderBuilder::primary_image(BitPix::I16, &[4, 4]).unwrap();
        for (k, v) in cards {
            hb = hb.card(k, v.clone(), None).unwrap();
        }
        let mut w = FitsWriter::create(&path).unwrap();
        w.write_image(&hb, &[0i16; 16]).unwrap();
        w.finish().unwrap();
        path
    }

    fn detect_from(name: &str, cards: &[(&str, Value)]) -> BayerPattern {
        let path = write_header(name, cards);
        let reader = FitsReader::open(&path).unwrap();
        detect(reader.primary_image().unwrap().header())
    }

    #[test]
    fn no_bayerpat_is_none() {
        assert_eq!(detect_from("none.fits", &[]), BayerPattern::None);
    }

    #[test]
    fn recognizes_each_pattern() {
        for (s, expected) in [
            ("RGGB", BayerPattern::Rggb),
            ("BGGR", BayerPattern::Bggr),
            ("GBRG", BayerPattern::Gbrg),
            ("GRBG", BayerPattern::Grbg),
        ] {
            let name = format!("{}.fits", s.to_lowercase());
            let got = detect_from(&name, &[("BAYERPAT", Value::String(s.to_string()))]);
            assert_eq!(got, expected);
        }
    }

    #[test]
    fn unrecognized_value_is_none() {
        let got = detect_from(
            "unrecognized.fits",
            &[("BAYERPAT", Value::String("XYZW".to_string()))],
        );
        assert_eq!(got, BayerPattern::None);
    }

    #[test]
    fn odd_x_offset_swaps_columns() {
        let got = detect_from(
            "xoff.fits",
            &[
                ("BAYERPAT", Value::String("RGGB".to_string())),
                ("XBAYROFF", Value::Integer(1)),
            ],
        );
        assert_eq!(got, BayerPattern::Grbg);
    }

    #[test]
    fn odd_both_offsets_is_diagonal_swap() {
        let got = detect_from(
            "xyoff.fits",
            &[
                ("BAYERPAT", Value::String("RGGB".to_string())),
                ("XBAYROFF", Value::Integer(1)),
                ("YBAYROFF", Value::Integer(1)),
            ],
        );
        assert_eq!(got, BayerPattern::Bggr);
    }
}
