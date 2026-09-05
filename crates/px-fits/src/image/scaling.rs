//! `BSCALE`/`BZERO`/`BLANK` as read from a header (FITS Standard 4.0
//! §4.4.2.5, §4.4.2.6). The physical value of a stored sample is
//! `physical = BZERO + BSCALE * raw`; `BLANK` (integer `BITPIX` only) names
//! the raw value that stands for "no data".

use crate::header::Header;

/// The scaling parameters for one image HDU, with FITS defaults already
/// resolved (`BSCALE = 1`, `BZERO = 0`, no `BLANK`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Scaling {
    pub bscale: f64,
    pub bzero: f64,
    /// Raw sentinel value for undefined integer pixels, if `BLANK` is present.
    pub blank: Option<i64>,
}

impl Default for Scaling {
    fn default() -> Self {
        Self {
            bscale: 1.0,
            bzero: 0.0,
            blank: None,
        }
    }
}

impl Scaling {
    pub fn from_header(header: &Header) -> Self {
        // A `BSCALE` of 0 is meaningless (it would erase the data); treat it
        // as absent, matching cfitsio's leniency.
        let bscale = header
            .get_f64("BSCALE")
            .filter(|v| *v != 0.0 && v.is_finite())
            .unwrap_or(1.0);
        let bzero = header
            .get_f64("BZERO")
            .filter(|v| v.is_finite())
            .unwrap_or(0.0);
        let blank = header.get_i64("BLANK");
        Self {
            bscale,
            bzero,
            blank,
        }
    }

    /// True when scaling is a no-op and raw values are already physical.
    ///
    /// The `float_cmp` these do is deliberate: `BSCALE`/`BZERO` are compared
    /// against the exact sentinel constants the FITS conventions define
    /// (`1`, `0`, `2^k`), not against computed results.
    #[allow(clippy::float_cmp)]
    pub fn is_identity(&self) -> bool {
        self.bscale == 1.0 && self.bzero == 0.0
    }

    /// When `BSCALE == 1` and `BZERO` is integral and fits in `i64`, scaling
    /// reduces to a single integer add — the fast path that keeps
    /// `read_full::<u16>()` on a `BZERO = 32768` frame off the float unit
    /// entirely (ADR 006 D6). Returns the offset to add to each raw sample.
    #[allow(clippy::float_cmp)]
    pub fn int_offset(&self) -> Option<i64> {
        if self.bscale == 1.0
            && self.bzero.fract() == 0.0
            && self.bzero >= i64::MIN as f64
            && self.bzero <= i64::MAX as f64
        {
            Some(self.bzero as i64)
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Card, Value};
    use crate::source::SliceSource;

    fn header_with(cards: Vec<Card>) -> Header {
        let mut bytes = Vec::new();
        for c in &cards {
            bytes.extend_from_slice(&c.to_bytes());
        }
        bytes.extend_from_slice(
            &Card::new("END", Value::Commentary(String::new()), None).to_bytes(),
        );
        let rem = bytes.len() % crate::block::BLOCK_SIZE;
        if rem != 0 {
            bytes.resize(bytes.len() + (crate::block::BLOCK_SIZE - rem), b' ');
        }
        Header::read(&SliceSource::new(bytes), 0).unwrap()
    }

    #[test]
    fn defaults_when_keywords_absent() {
        let s = Scaling::from_header(&header_with(vec![Card::new(
            "NAXIS",
            Value::Integer(0),
            None,
        )]));
        assert_eq!(s, Scaling::default());
        assert!(s.is_identity());
        assert_eq!(s.int_offset(), Some(0));
    }

    #[test]
    fn unsigned16_convention_is_an_integer_offset() {
        let s = Scaling::from_header(&header_with(vec![
            Card::new("BSCALE", Value::Float(1.0), None),
            Card::new("BZERO", Value::Float(32768.0), None),
        ]));
        assert!(!s.is_identity());
        assert_eq!(s.int_offset(), Some(32768));
    }

    #[test]
    fn real_bscale_has_no_integer_fast_path() {
        let s = Scaling::from_header(&header_with(vec![
            Card::new("BSCALE", Value::Float(0.5), None),
            Card::new("BZERO", Value::Float(7.0), None),
        ]));
        assert_eq!(s.int_offset(), None);
    }

    #[test]
    fn zero_bscale_is_ignored() {
        let s = Scaling::from_header(&header_with(vec![Card::new(
            "BSCALE",
            Value::Float(0.0),
            None,
        )]));
        assert_eq!(s.bscale, 1.0);
    }

    #[test]
    fn blank_is_captured() {
        let s = Scaling::from_header(&header_with(vec![Card::new(
            "BLANK",
            Value::Integer(-32768),
            None,
        )]));
        assert_eq!(s.blank, Some(-32768));
    }
}
