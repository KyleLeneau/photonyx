//! HDU discovery: locating each Header/Data Unit in a FITS file without
//! reading any data (FITS Standard 4.0 §3 "HDU structure", §7 "General
//! data-unit size formula" — the latter is what makes navigation possible
//! without interpreting what an HDU's data actually holds).

use crate::block::padded_len;
use crate::error::FitsError;
use crate::header::Header;
use crate::source::ByteSource;

/// The extension type declared by `XTENSION` (or `Primary` for the first
/// HDU, which has no `XTENSION` card). `Unknown` carries the raw value
/// verbatim rather than failing discovery — navigating past an HDU never
/// requires understanding its contents, so an exotic extension type is not
/// an error at this layer.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum HduKind {
    Primary,
    Image,
    AsciiTable,
    BinTable,
    Unknown(String),
}

impl HduKind {
    fn from_xtension(raw: &str) -> HduKind {
        match raw.trim() {
            "IMAGE" => HduKind::Image,
            "TABLE" => HduKind::AsciiTable,
            "BINTABLE" => HduKind::BinTable,
            other => HduKind::Unknown(other.to_string()),
        }
    }
}

/// One discovered HDU: its header (already fully parsed) and the byte
/// ranges of both its header and data units. `data_len` is the logical
/// (unpadded) size; `data_padded_len` is what's actually occupied on disk
/// (a multiple of the FITS block size) and is what navigation adds to
/// `data_offset` to find the next HDU.
#[derive(Debug, Clone)]
pub struct DiscoveredHdu {
    pub kind: HduKind,
    pub header: Header,
    pub header_offset: u64,
    pub header_len: u64,
    pub data_offset: u64,
    pub data_len: u64,
    pub data_padded_len: u64,
}

impl DiscoveredHdu {
    /// Where the next HDU's header would start, if there is one.
    pub fn next_header_offset(&self) -> u64 {
        self.data_offset + self.data_padded_len
    }
}

/// The FITS Standard's general data-unit size formula, valid uniformly for
/// images, tables, and (at the byte-range level) compressed images —
/// exactly what lets an HDU be skipped without knowing what kind it is:
///
/// ```text
/// Nbytes = 0                                              if NAXIS = 0
/// Nbytes = (|BITPIX| / 8) * GCOUNT * (PCOUNT + NAXIS1*...*NAXISn)   otherwise
/// ```
///
/// `PCOUNT` defaults to 0 and `GCOUNT` to 1 when absent, per the standard.
/// Every multiplication is checked; overflow is the ADR 006 O6 allocation
/// guard, surfacing as [`FitsError::NaxisOverflow`] before anything
/// downstream could try to size a buffer from a bogus declared length.
pub fn data_unit_len(header: &Header) -> Result<u64, FitsError> {
    let naxis = header.naxis()?;
    if naxis.is_empty() {
        return Ok(0);
    }

    let bitpix = header.bitpix()?;
    let bytes_per_pixel = bitpix.bytes_per_pixel() as u64;
    let pcount = header.get_i64("PCOUNT").unwrap_or(0).max(0) as u64;
    let gcount = header.get_i64("GCOUNT").unwrap_or(1).max(1) as u64;

    let element_count = naxis
        .iter()
        .try_fold(1u64, |acc, &n| acc.checked_mul(n))
        .ok_or(FitsError::NaxisOverflow)?;
    let per_group = element_count
        .checked_add(pcount)
        .ok_or(FitsError::NaxisOverflow)?;

    per_group
        .checked_mul(gcount)
        .and_then(|v| v.checked_mul(bytes_per_pixel))
        .ok_or(FitsError::NaxisOverflow)
}

/// Discovers one HDU starting at `offset`. Reads only that HDU's header
/// blocks — the data unit's byte range is computed arithmetically from the
/// header, never read.
pub fn discover_one<S: ByteSource + ?Sized>(
    source: &S,
    offset: u64,
    is_primary: bool,
) -> Result<DiscoveredHdu, FitsError> {
    let header = Header::read(source, offset)?;
    let header_len = header.byte_len();
    let data_offset = offset + header_len;
    let data_len = data_unit_len(&header)?;
    let data_padded_len = padded_len(data_len);

    let kind = if is_primary {
        HduKind::Primary
    } else {
        match header.get_string("XTENSION") {
            Some(x) => HduKind::from_xtension(&x),
            None => HduKind::Unknown(String::new()),
        }
    };

    Ok(DiscoveredHdu {
        kind,
        header,
        header_offset: offset,
        header_len,
        data_offset,
        data_len,
        data_padded_len,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Card, Value};
    use crate::source::SliceSource;

    fn header_bytes(cards: &[Card]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for c in cards {
            bytes.extend_from_slice(&c.to_bytes());
        }
        bytes.extend_from_slice(
            &Card::new("END", Value::Commentary(String::new()), None).to_bytes(),
        );
        let rem = bytes.len() % crate::block::BLOCK_SIZE;
        if rem != 0 {
            bytes.resize(bytes.len() + (crate::block::BLOCK_SIZE - rem), b' ');
        }
        bytes
    }

    #[test]
    fn data_unit_len_zero_naxis_is_zero() {
        let cards = vec![Card::new("NAXIS", Value::Integer(0), None)];
        let bytes = header_bytes(&cards);
        let source = SliceSource::new(bytes);
        let header = Header::read(&source, 0).unwrap();
        assert_eq!(data_unit_len(&header).unwrap(), 0);
    }

    #[test]
    fn data_unit_len_matches_bitpix_times_pixels() {
        let cards = vec![
            Card::new("BITPIX", Value::Integer(16), None),
            Card::new("NAXIS", Value::Integer(2), None),
            Card::new("NAXIS1", Value::Integer(10), None),
            Card::new("NAXIS2", Value::Integer(4), None),
        ];
        let bytes = header_bytes(&cards);
        let source = SliceSource::new(bytes);
        let header = Header::read(&source, 0).unwrap();
        // 10 * 4 pixels * 2 bytes/pixel (BITPIX=16) = 80 bytes.
        assert_eq!(data_unit_len(&header).unwrap(), 80);
    }

    #[test]
    fn data_unit_len_accounts_for_pcount_and_gcount() {
        let cards = vec![
            Card::new("BITPIX", Value::Integer(8), None),
            Card::new("NAXIS", Value::Integer(1), None),
            Card::new("NAXIS1", Value::Integer(10), None),
            Card::new("PCOUNT", Value::Integer(5), None),
            Card::new("GCOUNT", Value::Integer(2), None),
        ];
        let bytes = header_bytes(&cards);
        let source = SliceSource::new(bytes);
        let header = Header::read(&source, 0).unwrap();
        // (10 + 5) * 2 groups * 1 byte/pixel (BITPIX=8) = 30 bytes.
        assert_eq!(data_unit_len(&header).unwrap(), 30);
    }

    #[test]
    fn discover_one_computes_correct_offsets() {
        let cards = vec![
            Card::new("SIMPLE", Value::Logical(true), None),
            Card::new("BITPIX", Value::Integer(8), None),
            Card::new("NAXIS", Value::Integer(1), None),
            Card::new("NAXIS1", Value::Integer(10), None),
        ];
        let header_bytes = header_bytes(&cards);
        let header_len = header_bytes.len() as u64;
        let mut bytes = header_bytes;
        bytes.resize(bytes.len() + crate::block::BLOCK_SIZE, 0); // one padded data block

        let source = SliceSource::new(bytes);
        let hdu = discover_one(&source, 0, true).unwrap();

        assert_eq!(hdu.kind, HduKind::Primary);
        assert_eq!(hdu.header_offset, 0);
        assert_eq!(hdu.header_len, header_len);
        assert_eq!(hdu.data_offset, header_len);
        assert_eq!(hdu.data_len, 10);
        assert_eq!(hdu.data_padded_len, crate::block::BLOCK_SIZE as u64);
        assert_eq!(
            hdu.next_header_offset(),
            header_len + crate::block::BLOCK_SIZE as u64
        );
    }

    #[test]
    fn discover_one_extension_reports_kind_from_xtension() {
        let cards = vec![
            Card::new("XTENSION", Value::String("IMAGE".to_string()), None),
            Card::new("BITPIX", Value::Integer(16), None),
            Card::new("NAXIS", Value::Integer(0), None),
        ];
        let bytes = header_bytes(&cards);
        let source = SliceSource::new(bytes);
        let hdu = discover_one(&source, 0, false).unwrap();
        assert_eq!(hdu.kind, HduKind::Image);
    }
}
