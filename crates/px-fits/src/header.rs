//! FITS headers: reading the card sequence for one HDU from a [`ByteSource`]
//! and exposing typed keyword access (FITS Standard 4.0 §4 "Header").
//!
//! Reading is lazy in the sense Phase 2 depends on: `Header::read` touches
//! only the header's own blocks — it stops at the first `END` card and never
//! looks at the data unit that follows.

use std::collections::HashMap;

use crate::block::{BLOCK_SIZE, CARD_SIZE, CARDS_PER_BLOCK};
use crate::card::{Card, Value};
use crate::error::FitsError;
use crate::source::ByteSource;

use chrono::{DateTime, FixedOffset, NaiveDateTime, TimeZone, Utc};

/// The pixel data type declared by `BITPIX` (FITS Standard 4.0 §4.4.1.1).
/// Only the six standard values are valid; anything else is
/// [`FitsError::InvalidBitpix`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BitPix {
    U8,
    I16,
    I32,
    I64,
    F32,
    F64,
}

impl BitPix {
    pub fn from_i64(value: i64) -> Result<Self, FitsError> {
        match value {
            8 => Ok(BitPix::U8),
            16 => Ok(BitPix::I16),
            32 => Ok(BitPix::I32),
            64 => Ok(BitPix::I64),
            -32 => Ok(BitPix::F32),
            -64 => Ok(BitPix::F64),
            other => Err(FitsError::InvalidBitpix(other)),
        }
    }

    /// The raw `BITPIX` value this variant corresponds to.
    pub fn as_i64(&self) -> i64 {
        match self {
            BitPix::U8 => 8,
            BitPix::I16 => 16,
            BitPix::I32 => 32,
            BitPix::I64 => 64,
            BitPix::F32 => -32,
            BitPix::F64 => -64,
        }
    }

    pub fn bytes_per_pixel(&self) -> usize {
        match self {
            BitPix::U8 => 1,
            BitPix::I16 => 2,
            BitPix::I32 | BitPix::F32 => 4,
            BitPix::I64 | BitPix::F64 => 8,
        }
    }
}

/// One HDU's header: an ordered card sequence plus a keyword index for O(1)
/// lookup. `CONTINUE` long-string chains (the OGIP convention) are already
/// merged into a single logical card by the time a `Header` is constructed —
/// callers never see raw `CONTINUE` cards.
#[derive(Debug, Clone)]
pub struct Header {
    cards: Vec<Card>,
    /// Keyword (uppercased) -> index into `cards`, first occurrence wins,
    /// matching common FITS reader behaviour when a keyword is (against the
    /// standard's intent, but not uncommon in practice) repeated.
    index: HashMap<String, usize>,
    /// Total size in bytes of the header unit on disk, including the block
    /// containing `END` and any trailing pad — always a multiple of
    /// [`BLOCK_SIZE`].
    byte_len: u64,
}

impl Header {
    /// Reads a header starting at `offset` in `source`, stopping at the
    /// first `END` card. Returns [`FitsError::MissingEnd`] if the source is
    /// exhausted first.
    pub fn read<S: ByteSource + ?Sized>(source: &S, offset: u64) -> Result<Self, FitsError> {
        let mut raw_cards: Vec<Card> = Vec::new();
        let mut blocks_read: u64 = 0;
        let mut block_buf = [0u8; BLOCK_SIZE];

        loop {
            let block_offset = offset + blocks_read * BLOCK_SIZE as u64;
            source
                .read_exact_at(&mut block_buf, block_offset)
                .map_err(|e| {
                    if e.kind() == std::io::ErrorKind::UnexpectedEof {
                        FitsError::MissingEnd
                    } else {
                        FitsError::Io(e)
                    }
                })?;
            blocks_read += 1;

            let mut found_end = false;
            for i in 0..CARDS_PER_BLOCK {
                let start = i * CARD_SIZE;
                let mut card_bytes = [0u8; CARD_SIZE];
                card_bytes.copy_from_slice(&block_buf[start..start + CARD_SIZE]);
                let card = Card::parse(&card_bytes);
                if card.keyword.eq_ignore_ascii_case("END") {
                    found_end = true;
                    break;
                }
                raw_cards.push(card);
            }
            if found_end {
                break;
            }
        }

        let cards = merge_continuations(raw_cards);
        let mut index = HashMap::with_capacity(cards.len());
        for (i, card) in cards.iter().enumerate() {
            index.entry(card.keyword.to_ascii_uppercase()).or_insert(i);
        }

        Ok(Header {
            cards,
            index,
            byte_len: blocks_read * BLOCK_SIZE as u64,
        })
    }

    /// Total size in bytes of this header unit on disk (a multiple of
    /// [`BLOCK_SIZE`]).
    pub fn byte_len(&self) -> u64 {
        self.byte_len
    }

    /// All cards in file order, `CONTINUE` chains already merged.
    pub fn cards(&self) -> &[Card] {
        &self.cards
    }

    pub fn get(&self, keyword: &str) -> Option<&Card> {
        self.index
            .get(&keyword.to_ascii_uppercase())
            .map(|&i| &self.cards[i])
    }

    pub fn get_string(&self, keyword: &str) -> Option<&str> {
        match &self.get(keyword)?.value {
            Value::String(s) => Some(s.as_str()),
            _ => None,
        }
    }

    pub fn get_i64(&self, keyword: &str) -> Option<i64> {
        match &self.get(keyword)?.value {
            Value::Integer(i) => Some(*i),
            _ => None,
        }
    }

    pub fn get_f64(&self, keyword: &str) -> Option<f64> {
        match &self.get(keyword)?.value {
            Value::Float(f) => Some(*f),
            Value::Integer(i) => Some(*i as f64),
            _ => None,
        }
    }

    pub fn get_bool(&self, keyword: &str) -> Option<bool> {
        match &self.get(keyword)?.value {
            Value::Logical(b) => Some(*b),
            _ => None,
        }
    }

    pub fn get_date_utc(&self, keyword: &str) -> Option<DateTime<FixedOffset>> {
        let s = self.get_string(keyword)?;
        DateTime::parse_from_rfc3339(s).ok().or_else(|| {
            NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
                .ok()
                .map(|ndt| Utc.from_utc_datetime(&ndt).fixed_offset())
        })
    }

    /// The declared `BITPIX`, validated against the six standard values.
    pub fn bitpix(&self) -> Result<BitPix, FitsError> {
        let raw = self.get_i64("BITPIX").ok_or(FitsError::MissingPrimaryHdu)?;
        BitPix::from_i64(raw)
    }

    /// The declared axis lengths (`NAXIS1..NAXISn`), fastest-varying axis
    /// first, per `NAXIS`. Validates that every `NAXISn` is non-negative
    /// (FITS Standard 4.0 §4.4.1.2) and that their product does not overflow
    /// `u64` — the latter is the allocation guard from ADR 006 O6, catching
    /// implausible declared dimensions before anything downstream tries to
    /// size a buffer from them.
    pub fn naxis(&self) -> Result<Vec<u64>, FitsError> {
        let count = self.get_i64("NAXIS").unwrap_or(0);
        let count = usize::try_from(count).unwrap_or(0);

        let mut axes = Vec::with_capacity(count);
        let mut total: u64 = 1;
        for i in 1..=count {
            let key = format!("NAXIS{i}");
            let value = self.get_i64(&key).unwrap_or(0);
            if value < 0 {
                return Err(FitsError::NegativeNaxis { axis: i, value });
            }
            let value = value as u64;
            total = total
                .checked_mul(value.max(1))
                .ok_or(FitsError::NaxisOverflow)?;
            axes.push(value);
        }
        // A zero-length axis makes the whole product zero regardless of the
        // `.max(1)` guard above (which exists only to avoid short-circuiting
        // overflow detection on legitimate zero-length axes).
        if axes.contains(&0) {
            total = 0;
        }
        let _ = total; // computed for its overflow-detecting side effect
        Ok(axes)
    }
}

/// Merges `CONTINUE` chains (OGIP long-string convention) into the
/// initiating card: a string value ending in `&` is continued by the next
/// `CONTINUE` card's string value, itself possibly `&`-terminated, and so
/// on. The trailing `&` markers are stripped from the merged result.
fn merge_continuations(raw: Vec<Card>) -> Vec<Card> {
    let mut out: Vec<Card> = Vec::with_capacity(raw.len());
    let mut iter = raw.into_iter().peekable();

    while let Some(mut card) = iter.next() {
        if let Value::String(s) = &card.value
            && let Some(stripped) = s.strip_suffix('&')
        {
            let mut merged = stripped.to_string();
            let mut still_continuing = true;
            while still_continuing {
                still_continuing = false;
                if let Some(next) = iter.peek()
                    && next.keyword.eq_ignore_ascii_case("CONTINUE")
                {
                    let next = iter.next().unwrap();
                    if let Value::String(next_s) = next.value {
                        match next_s.strip_suffix('&') {
                            Some(chunk) => {
                                merged.push_str(chunk);
                                still_continuing = true;
                            }
                            None => merged.push_str(&next_s),
                        }
                    }
                }
            }
            card.value = Value::String(merged);
        }
        out.push(card);
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::source::SliceSource;

    fn header_bytes(cards: &[Card]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for c in cards {
            bytes.extend_from_slice(&c.to_bytes());
        }
        bytes.extend_from_slice(
            &Card::new("END", Value::Commentary(String::new()), None).to_bytes(),
        );
        let rem = bytes.len() % BLOCK_SIZE;
        if rem != 0 {
            bytes.resize(bytes.len() + (BLOCK_SIZE - rem), b' ');
        }
        bytes
    }

    #[test]
    fn reads_simple_header_and_stops_at_end() {
        let cards = vec![
            Card::new("SIMPLE", Value::Logical(true), None),
            Card::new("BITPIX", Value::Integer(16), None),
            Card::new("NAXIS", Value::Integer(2), None),
            Card::new("NAXIS1", Value::Integer(64), None),
            Card::new("NAXIS2", Value::Integer(48), None),
        ];
        let bytes = header_bytes(&cards);
        let source = SliceSource::new(bytes);
        let header = Header::read(&source, 0).unwrap();

        assert_eq!(header.get_bool("SIMPLE"), Some(true));
        assert_eq!(header.get_i64("BITPIX"), Some(16));
        assert_eq!(header.bitpix().unwrap(), BitPix::I16);
        assert_eq!(header.naxis().unwrap(), vec![64, 48]);
        assert_eq!(header.byte_len(), BLOCK_SIZE as u64);
    }

    #[test]
    fn reads_header_spanning_multiple_blocks() {
        let mut cards = vec![Card::new("SIMPLE", Value::Logical(true), None)];
        for i in 0..40 {
            cards.push(Card::new(
                "COMMENT",
                Value::Commentary(format!("line {i}")),
                None,
            ));
        }
        let bytes = header_bytes(&cards);
        assert!(
            bytes.len() > BLOCK_SIZE,
            "test fixture should span >1 block"
        );
        let source = SliceSource::new(bytes);
        let header = Header::read(&source, 0).unwrap();

        assert_eq!(header.byte_len(), BLOCK_SIZE as u64 * 2);
        assert_eq!(header.get_bool("SIMPLE"), Some(true));
    }

    #[test]
    fn missing_end_before_eof_errors() {
        // A single well-formed block with no END card anywhere in it.
        let mut bytes = vec![b' '; BLOCK_SIZE];
        let card = Card::new("SIMPLE", Value::Logical(true), None).to_bytes();
        bytes[0..CARD_SIZE].copy_from_slice(&card);
        let source = SliceSource::new(bytes);

        let err = Header::read(&source, 0).unwrap_err();
        assert!(matches!(err, FitsError::MissingEnd));
    }

    #[test]
    fn invalid_bitpix_is_rejected_by_typed_accessor() {
        let cards = vec![Card::new("BITPIX", Value::Integer(3), None)];
        let bytes = header_bytes(&cards);
        let source = SliceSource::new(bytes);
        let header = Header::read(&source, 0).unwrap();

        let err = header.bitpix().unwrap_err();
        assert!(matches!(err, FitsError::InvalidBitpix(3)));
    }

    #[test]
    fn negative_naxis_is_rejected() {
        let cards = vec![
            Card::new("NAXIS", Value::Integer(1), None),
            Card::new("NAXIS1", Value::Integer(-10), None),
        ];
        let bytes = header_bytes(&cards);
        let source = SliceSource::new(bytes);
        let header = Header::read(&source, 0).unwrap();

        let err = header.naxis().unwrap_err();
        assert!(matches!(
            err,
            FitsError::NegativeNaxis {
                axis: 1,
                value: -10
            }
        ));
    }

    #[test]
    fn naxis_overflow_is_rejected() {
        let cards = vec![
            Card::new("NAXIS", Value::Integer(2), None),
            Card::new("NAXIS1", Value::Integer(999_999_999_999), None),
            Card::new("NAXIS2", Value::Integer(999_999_999_999), None),
        ];
        let bytes = header_bytes(&cards);
        let source = SliceSource::new(bytes);
        let header = Header::read(&source, 0).unwrap();

        let err = header.naxis().unwrap_err();
        assert!(matches!(err, FitsError::NaxisOverflow));
    }

    #[test]
    fn malformed_card_is_queryable_as_invalid_not_an_error() {
        // Permissive parsing (ADR 006 D-series): one bad card doesn't fail
        // the whole header. Build the raw bytes directly since Card::new
        // can't construct an Invalid on purpose (see card_proptest.rs docs).
        let mut bytes = vec![b' '; BLOCK_SIZE];
        let mut bad = String::from("BADCARD!!!!not a valid value syntax at all,,,,");
        bad.push_str(&" ".repeat(CARD_SIZE.saturating_sub(bad.len())));
        bytes[0..CARD_SIZE].copy_from_slice(&bad.as_bytes()[..CARD_SIZE]);
        let end = Card::new("END", Value::Commentary(String::new()), None).to_bytes();
        bytes[CARD_SIZE..CARD_SIZE * 2].copy_from_slice(&end);
        let source = SliceSource::new(bytes);

        let header = Header::read(&source, 0).unwrap();
        let card = header
            .get("BADCARD!")
            .expect("malformed card still present");
        assert!(matches!(card.value, Value::Invalid(_)));
    }

    #[test]
    fn merges_continue_chain_into_one_logical_string() {
        let cards = vec![
            Card::new(
                "LONGSTR",
                Value::String("part one &".to_string()),
                Some("desc".to_string()),
            ),
            Card::new("CONTINUE", Value::String("part two &".to_string()), None),
            Card::new("CONTINUE", Value::String("part three".to_string()), None),
        ];
        let bytes = header_bytes(&cards);
        let source = SliceSource::new(bytes);
        let header = Header::read(&source, 0).unwrap();

        assert_eq!(
            header.get_string("LONGSTR"),
            Some("part one part two part three")
        );
        // CONTINUE cards are absorbed, not separately queryable.
        assert!(
            header
                .cards()
                .iter()
                .all(|c| !c.keyword.eq_ignore_ascii_case("CONTINUE"))
        );
    }

    #[test]
    fn duplicate_keyword_first_occurrence_wins() {
        let cards = vec![
            Card::new("FOO", Value::Integer(1), None),
            Card::new("FOO", Value::Integer(2), None),
        ];
        let bytes = header_bytes(&cards);
        let source = SliceSource::new(bytes);
        let header = Header::read(&source, 0).unwrap();

        assert_eq!(header.get_i64("FOO"), Some(1));
    }
}
