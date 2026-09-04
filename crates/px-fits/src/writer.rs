//! FITS writing (ADR 006 Phase 5): [`HeaderBuilder`], [`FitsWriter`], the
//! streaming [`ImageWriter`], and [`update_header`].
//!
//! Headers are emitted in **fixed format** for every keyword the standard
//! defines that way (FITS Standard 4.0 §4.2.1): keyword in bytes 1–8, `= `
//! in 9–10, value right-justified through byte 30, optional ` / comment`.
//! Data units are padded to a 2880-byte boundary with zero bytes; headers
//! with ASCII spaces (§3.3.2). `HeaderBuilder` enforces the mandatory
//! keyword order (`SIMPLE`/`XTENSION`, `BITPIX`, `NAXIS`, `NAXISn`,
//! `PCOUNT`/`GCOUNT`, …, `END`) by construction (D7), so a caller cannot
//! emit a structurally invalid header by accident.

use std::fs::{File, OpenOptions};
use std::io::{self, BufWriter, Read, Seek, SeekFrom, Write};
use std::marker::PhantomData;
use std::path::Path;

use crate::block::{BLOCK_SIZE, CARD_SIZE};
use crate::card::{Card, Value};
use crate::error::FitsError;
use crate::header::BitPix;
use crate::image::Pixel;

// ---------------------------------------------------------------------------
// Fixed-format card serialization
// ---------------------------------------------------------------------------

fn blank_card() -> [u8; CARD_SIZE] {
    [b' '; CARD_SIZE]
}

fn card_from_str(line: &str) -> [u8; CARD_SIZE] {
    let mut out = blank_card();
    let bytes = line.as_bytes();
    let n = bytes.len().min(CARD_SIZE);
    out[..n].copy_from_slice(&bytes[..n]);
    out
}

/// `KEYWORD = <value right-justified to byte 30>[ / comment]`.
fn fixed_kv(keyword: &str, value_field: &str, comment: Option<&str>) -> [u8; CARD_SIZE] {
    let mut line = format!("{keyword:<8}= {value_field:>20}");
    if let Some(c) = comment.filter(|c| !c.is_empty()) {
        line.push_str(" / ");
        line.push_str(c);
    }
    card_from_str(&line)
}

/// FITS fixed-format float: always carries a `.` or exponent so it re-parses
/// as a float and not an integer, and uses an uppercase `E` exponent marker
/// (§4.2.4). `{:?}` on `f64` guarantees a decimal point for integral values.
fn format_float(v: f64) -> String {
    let mut s = format!("{v:?}");
    if !s.contains(['.', 'e', 'E']) {
        s.push_str(".0");
    }
    s.replace('e', "E")
}

/// A single string-valued card. Returns `None` if the quoted value plus its
/// comment cannot fit in one 80-byte card — `HeaderBuilder` rejects that
/// case up front rather than silently truncating (CONTINUE-chain *writing*
/// is not part of Phase 5).
fn string_card(keyword: &str, value: &str, comment: Option<&str>) -> Option<[u8; CARD_SIZE]> {
    let escaped = value.replace('\'', "''");
    // The standard requires at least 8 characters between the quotes.
    let inner = if escaped.chars().count() < 8 {
        format!("{escaped:<8}")
    } else {
        escaped
    };
    let mut line = format!("{keyword:<8}= '{inner}'");
    if let Some(c) = comment.filter(|c| !c.is_empty()) {
        while line.len() < 30 {
            line.push(' ');
        }
        line.push_str(" / ");
        line.push_str(c);
    }
    if line.len() > CARD_SIZE {
        return None;
    }
    Some(card_from_str(&line))
}

/// Serializes one keyword/value/comment triple to a fixed-format 80-byte
/// card. Errors (rather than truncating) on a string too long for one card.
fn serialize_card(
    keyword: &str,
    value: &Value,
    comment: Option<&str>,
) -> Result<[u8; CARD_SIZE], FitsError> {
    let card = match value {
        Value::Logical(b) => fixed_kv(keyword, if *b { "T" } else { "F" }, comment),
        Value::Integer(i) => fixed_kv(keyword, &i.to_string(), comment),
        Value::Float(f) => fixed_kv(keyword, &format_float(*f), comment),
        Value::Undefined => fixed_kv(keyword, "", comment),
        Value::Complex(re, im) => fixed_kv(
            keyword,
            &format!("({}, {})", format_float(*re), format_float(*im)),
            comment,
        ),
        Value::String(s) => string_card(keyword, s, comment).ok_or_else(|| {
            FitsError::Processing(format!(
                "string value for {keyword} is too long for one card (CONTINUE writing \
                 is not supported yet)"
            ))
        })?,
        Value::Commentary(text) => card_from_str(&format!("{keyword:<8}{text}")),
        Value::Invalid(raw) => card_from_str(&format!("{keyword:<8}{raw}")),
    };
    Ok(card)
}

fn end_card() -> [u8; CARD_SIZE] {
    card_from_str("END")
}

/// Flattens cards, appends `END`, and pads to a whole number of 2880-byte
/// blocks with ASCII spaces.
fn assemble_header(cards: &[[u8; CARD_SIZE]]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity((cards.len() + 1) * CARD_SIZE);
    for c in cards {
        bytes.extend_from_slice(c);
    }
    bytes.extend_from_slice(&end_card());
    let rem = bytes.len() % BLOCK_SIZE;
    if rem != 0 {
        bytes.resize(bytes.len() + (BLOCK_SIZE - rem), b' ');
    }
    bytes
}

// ---------------------------------------------------------------------------
// Keyword validation
// ---------------------------------------------------------------------------

/// Keywords that describe the physical structure of the HDU. `HeaderBuilder`
/// owns these; a caller may not set them directly, and `update_header` may
/// not edit or remove them (doing so would invalidate the data-unit size).
fn is_structural_keyword(keyword: &str) -> bool {
    let k = keyword.to_ascii_uppercase();
    matches!(
        k.as_str(),
        "SIMPLE" | "BITPIX" | "NAXIS" | "XTENSION" | "END" | "PCOUNT" | "GCOUNT" | "EXTEND"
    ) || (k.starts_with("NAXIS") && k[5..].chars().all(|c| c.is_ascii_digit()) && k.len() > 5)
}

/// A user keyword must be 1–8 chars of `A–Z`, `0–9`, `-`, `_` (§4.1.2.1), or
/// one of the commentary keywords. Rejects lowercase, non-ASCII, and
/// over-length keywords.
fn validate_user_keyword(keyword: &str) -> Result<(), FitsError> {
    if matches!(keyword, "COMMENT" | "HISTORY" | "") {
        return Ok(());
    }
    let ok = (1..=8).contains(&keyword.len())
        && keyword
            .bytes()
            .all(|b| b.is_ascii_uppercase() || b.is_ascii_digit() || b == b'-' || b == b'_');
    if !ok {
        return Err(FitsError::Processing(format!(
            "invalid FITS keyword {keyword:?}: must be 1-8 chars of A-Z, 0-9, '-', '_'"
        )));
    }
    if is_structural_keyword(keyword) {
        return Err(FitsError::Processing(format!(
            "{keyword} is a structural keyword and is managed by the writer"
        )));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// HeaderBuilder
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum HeaderKind {
    Primary,
    ImageExt,
}

/// Builds one image HDU's header. Construct with [`primary_image`] or
/// [`image_ext`], then attach extra cards with [`card`] / the typed
/// `set_*` helpers. The mandatory keywords are emitted automatically in the
/// order and format the standard requires.
///
/// [`primary_image`]: HeaderBuilder::primary_image
/// [`image_ext`]: HeaderBuilder::image_ext
/// [`card`]: HeaderBuilder::card
#[derive(Debug, Clone)]
pub struct HeaderBuilder {
    kind: HeaderKind,
    bitpix: BitPix,
    axes: Vec<u64>,
    extra: Vec<(String, Value, Option<String>)>,
}

impl HeaderBuilder {
    fn new(kind: HeaderKind, bitpix: BitPix, axes: &[u64]) -> Result<Self, FitsError> {
        // Overflow guard (ADR 006 O6): the declared element count times the
        // pixel size must fit in u64.
        let mut count: u64 = 1;
        for &n in axes {
            count = count
                .checked_mul(n.max(1))
                .ok_or(FitsError::NaxisOverflow)?;
        }
        count
            .checked_mul(bitpix.bytes_per_pixel() as u64)
            .ok_or(FitsError::NaxisOverflow)?;
        Ok(Self {
            kind,
            bitpix,
            axes: axes.to_vec(),
            extra: Vec::new(),
        })
    }

    /// A primary-HDU image header. `axes` is `NAXIS1..NAXISn`, fastest-varying
    /// axis first; empty `axes` means `NAXIS = 0` (a data-less primary).
    pub fn primary_image(bitpix: BitPix, axes: &[u64]) -> Result<Self, FitsError> {
        Self::new(HeaderKind::Primary, bitpix, axes)
    }

    /// An `IMAGE`-extension header.
    pub fn image_ext(bitpix: BitPix, axes: &[u64]) -> Result<Self, FitsError> {
        Self::new(HeaderKind::ImageExt, bitpix, axes)
    }

    /// Appends a card after the mandatory block. A later `card` with a
    /// keyword already present replaces the earlier one (except `COMMENT`/
    /// `HISTORY`, which accumulate).
    pub fn card(
        mut self,
        keyword: &str,
        value: Value,
        comment: Option<&str>,
    ) -> Result<Self, FitsError> {
        validate_user_keyword(keyword)?;
        // Reject up front anything that can't be serialized to one card.
        serialize_card(keyword, &value, comment)?;

        let comment = comment.map(str::to_string);
        if matches!(keyword, "COMMENT" | "HISTORY") {
            self.extra.push((keyword.to_string(), value, comment));
        } else if let Some(slot) = self
            .extra
            .iter_mut()
            .find(|(k, _, _)| k.eq_ignore_ascii_case(keyword))
        {
            slot.1 = value;
            slot.2 = comment;
        } else {
            self.extra.push((keyword.to_string(), value, comment));
        }
        Ok(self)
    }

    pub fn set_i64(
        self,
        keyword: &str,
        value: i64,
        comment: Option<&str>,
    ) -> Result<Self, FitsError> {
        self.card(keyword, Value::Integer(value), comment)
    }

    pub fn set_f64(
        self,
        keyword: &str,
        value: f64,
        comment: Option<&str>,
    ) -> Result<Self, FitsError> {
        self.card(keyword, Value::Float(value), comment)
    }

    pub fn set_bool(
        self,
        keyword: &str,
        value: bool,
        comment: Option<&str>,
    ) -> Result<Self, FitsError> {
        self.card(keyword, Value::Logical(value), comment)
    }

    pub fn set_str(
        self,
        keyword: &str,
        value: &str,
        comment: Option<&str>,
    ) -> Result<Self, FitsError> {
        self.card(keyword, Value::String(value.to_string()), comment)
    }

    pub fn comment(self, text: &str) -> Result<Self, FitsError> {
        self.card("COMMENT", Value::Commentary(text.to_string()), None)
    }

    pub fn bitpix(&self) -> BitPix {
        self.bitpix
    }

    /// `NAXIS1` (the fastest-varying axis extent), i.e. one row's pixel
    /// count. Zero when `NAXIS = 0`.
    fn row_len(&self) -> usize {
        self.axes.first().copied().unwrap_or(0) as usize
    }

    /// Number of rows in the data unit (`∏ NAXIS2..NAXISn`). Zero when there
    /// is no data.
    fn total_rows(&self) -> usize {
        if self.axes.is_empty() || self.axes.contains(&0) {
            0
        } else {
            self.axes[1..].iter().map(|&n| n as usize).product()
        }
    }

    fn mandatory_cards(&self) -> Result<Vec<[u8; CARD_SIZE]>, FitsError> {
        let mut cards = Vec::new();
        match self.kind {
            HeaderKind::Primary => {
                cards.push(fixed_kv("SIMPLE", "T", Some("conforms to FITS standard")));
            }
            HeaderKind::ImageExt => {
                cards.push(
                    string_card("XTENSION", "IMAGE", Some("image extension"))
                        .expect("'IMAGE' always fits"),
                );
            }
        }
        cards.push(fixed_kv(
            "BITPIX",
            &self.bitpix.as_i64().to_string(),
            Some("bits per data value"),
        ));
        cards.push(fixed_kv(
            "NAXIS",
            &self.axes.len().to_string(),
            Some("number of data axes"),
        ));
        for (i, n) in self.axes.iter().enumerate() {
            cards.push(fixed_kv(
                &format!("NAXIS{}", i + 1),
                &n.to_string(),
                Some("axis length"),
            ));
        }
        match self.kind {
            HeaderKind::Primary => {
                cards.push(fixed_kv("EXTEND", "T", Some("file may contain extensions")));
            }
            HeaderKind::ImageExt => {
                cards.push(fixed_kv("PCOUNT", "0", Some("no group parameters")));
                cards.push(fixed_kv("GCOUNT", "1", Some("one data group")));
            }
        }
        Ok(cards)
    }

    /// The full header block(s): mandatory cards, then the extra cards, then
    /// `END`, padded to a block boundary with spaces.
    fn serialize(&self) -> Result<Vec<u8>, FitsError> {
        let mut cards = self.mandatory_cards()?;
        for (k, v, c) in &self.extra {
            cards.push(serialize_card(k, v, c.as_deref())?);
        }
        Ok(assemble_header(&cards))
    }
}

// ---------------------------------------------------------------------------
// FitsWriter
// ---------------------------------------------------------------------------

/// Writes a FITS file HDU by HDU. The first HDU must come from
/// [`HeaderBuilder::primary_image`]; any later HDU from
/// [`HeaderBuilder::image_ext`].
#[derive(Debug)]
pub struct FitsWriter<W: Write> {
    inner: W,
    hdus_written: usize,
}

impl FitsWriter<BufWriter<File>> {
    /// Creates (truncating) `path` for writing. Uses a 1 MiB write buffer so
    /// row-at-a-time streaming (`begin_image`) doesn't turn into one `write`
    /// syscall per image row.
    pub fn create(path: impl AsRef<Path>) -> Result<Self, FitsError> {
        Ok(Self::new(BufWriter::with_capacity(
            1 << 20,
            File::create(path)?,
        )))
    }
}

impl<W: Write> FitsWriter<W> {
    pub fn new(inner: W) -> Self {
        Self {
            inner,
            hdus_written: 0,
        }
    }

    /// Writes a complete image HDU: header, then `data` as big-endian
    /// samples, then block padding. `data.len()` must equal the product of
    /// the header's axes.
    pub fn write_image<T: Pixel>(
        &mut self,
        header: &HeaderBuilder,
        data: &[T],
    ) -> Result<(), FitsError> {
        let expected = header.row_len().saturating_mul(header.total_rows());
        if data.len() != expected {
            return Err(FitsError::BufferLenMismatch {
                expected,
                got: data.len(),
            });
        }
        let mut image = self.begin_image::<T>(header)?;
        if header.row_len() > 0 {
            for row in data.chunks(header.row_len()) {
                image.write_row(row)?;
            }
        }
        image.finish()
    }

    /// Writes an image HDU's header and returns a row-at-a-time writer, so
    /// peak heap stays at one row regardless of image size (ADR 006 P5-T3).
    pub fn begin_image<T: Pixel>(
        &mut self,
        header: &HeaderBuilder,
    ) -> Result<ImageWriter<'_, W, T>, FitsError> {
        if T::STORAGE_BITPIX != Some(header.bitpix.as_i64()) {
            return Err(FitsError::Processing(format!(
                "pixel type is not the natural storage type for BITPIX {}",
                header.bitpix.as_i64()
            )));
        }
        let first = self.hdus_written == 0;
        let is_primary = header.kind == HeaderKind::Primary;
        if first != is_primary {
            return Err(FitsError::Processing(
                if first {
                    "the first HDU must be a primary header (HeaderBuilder::primary_image)"
                } else {
                    "HDUs after the first must be IMAGE extensions (HeaderBuilder::image_ext)"
                }
                .to_string(),
            ));
        }

        let bytes = header.serialize()?;
        self.inner.write_all(&bytes)?;
        self.hdus_written += 1;

        let px = header.bitpix.bytes_per_pixel();
        Ok(ImageWriter {
            inner: &mut self.inner,
            px_bytes: px,
            row_len: header.row_len(),
            rows_left: header.total_rows(),
            data_bytes: 0,
            scratch: vec![0u8; header.row_len() * px],
            _t: PhantomData,
        })
    }

    /// Flushes and returns the underlying writer.
    pub fn finish(mut self) -> Result<W, FitsError> {
        self.inner.flush()?;
        Ok(self.inner)
    }
}

/// Row-at-a-time image data writer from [`FitsWriter::begin_image`].
pub struct ImageWriter<'w, W: Write, T: Pixel> {
    inner: &'w mut W,
    px_bytes: usize,
    row_len: usize,
    rows_left: usize,
    data_bytes: u64,
    scratch: Vec<u8>,
    _t: PhantomData<fn(T)>,
}

impl<W: Write, T: Pixel> ImageWriter<'_, W, T> {
    /// Encodes and writes one row (`row.len()` must equal `NAXIS1`).
    pub fn write_row(&mut self, row: &[T]) -> Result<(), FitsError> {
        if self.rows_left == 0 {
            return Err(FitsError::Processing(
                "every image row has already been written".to_string(),
            ));
        }
        if row.len() != self.row_len {
            return Err(FitsError::BufferLenMismatch {
                expected: self.row_len,
                got: row.len(),
            });
        }
        for (i, &v) in row.iter().enumerate() {
            v.encode_be(&mut self.scratch[i * self.px_bytes..]);
        }
        self.inner.write_all(&self.scratch)?;
        self.data_bytes += self.scratch.len() as u64;
        self.rows_left -= 1;
        Ok(())
    }

    /// Pads the data unit to a block boundary with zero bytes. Errors if any
    /// row is still owed.
    pub fn finish(self) -> Result<(), FitsError> {
        if self.rows_left != 0 {
            return Err(FitsError::Processing(format!(
                "{} image row(s) not written",
                self.rows_left
            )));
        }
        let rem = (self.data_bytes % BLOCK_SIZE as u64) as usize;
        if rem != 0 {
            let pad = vec![0u8; BLOCK_SIZE - rem];
            self.inner.write_all(&pad)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// update_header
// ---------------------------------------------------------------------------

/// One edit for [`update_header`].
#[derive(Debug, Clone)]
pub enum CardEdit {
    /// Add the keyword, or replace its value/comment if already present.
    Set {
        keyword: String,
        value: Value,
        comment: Option<String>,
    },
    /// Remove the keyword if present (no error if absent).
    Remove { keyword: String },
}

impl CardEdit {
    pub fn set(keyword: impl Into<String>, value: Value) -> Self {
        CardEdit::Set {
            keyword: keyword.into(),
            value,
            comment: None,
        }
    }

    pub fn set_with_comment(
        keyword: impl Into<String>,
        value: Value,
        comment: impl Into<String>,
    ) -> Self {
        CardEdit::Set {
            keyword: keyword.into(),
            value,
            comment: Some(comment.into()),
        }
    }

    pub fn remove(keyword: impl Into<String>) -> Self {
        CardEdit::Remove {
            keyword: keyword.into(),
        }
    }
}

/// Applies `edits` to HDU `hdu_index`'s header of the FITS file at `path`,
/// rewriting that header in canonical fixed format.
///
/// If the edited header occupies the same number of 2880-byte blocks as the
/// original it is patched in place; otherwise the whole file is rebuilt in a
/// sibling temp file and atomically renamed over `path`, so a failure never
/// leaves a partially written file at the original path (ADR 006 P5-T4).
///
/// Structural keywords (`SIMPLE`, `BITPIX`, `NAXIS`, `NAXISn`, `XTENSION`,
/// `PCOUNT`, `GCOUNT`, `EXTEND`, `END`) may not be edited or removed.
pub fn update_header(path: &Path, hdu_index: usize, edits: &[CardEdit]) -> Result<(), FitsError> {
    use crate::reader::FitsReader;

    let reader = FitsReader::open(path)?;
    let hdu = reader.hdu(hdu_index)?;
    let header_offset = hdu.header_offset;
    let old_header_len = hdu.header_len as usize;

    let mut cards: Vec<Card> = hdu.header.cards().to_vec();
    for edit in edits {
        match edit {
            CardEdit::Set {
                keyword,
                value,
                comment,
            } => {
                reject_structural(keyword)?;
                validate_user_keyword(keyword)?;
                let new = Card::new(keyword.clone(), value.clone(), comment.clone());
                match cards
                    .iter()
                    .position(|c| c.keyword.eq_ignore_ascii_case(keyword))
                {
                    Some(i) => cards[i] = new,
                    None => insert_before_trailing_commentary(&mut cards, new),
                }
            }
            CardEdit::Remove { keyword } => {
                reject_structural(keyword)?;
                cards.retain(|c| !c.keyword.eq_ignore_ascii_case(keyword));
            }
        }
    }

    let mut serialized: Vec<[u8; CARD_SIZE]> = Vec::with_capacity(cards.len());
    for c in &cards {
        serialized.push(serialize_card(&c.keyword, &c.value, c.comment.as_deref())?);
    }
    let new_header = assemble_header(&serialized);

    if new_header.len() == old_header_len {
        let mut file = OpenOptions::new().write(true).open(path)?;
        file.seek(SeekFrom::Start(header_offset))?;
        file.write_all(&new_header)?;
        file.sync_all()?;
        return Ok(());
    }

    rewrite_with_new_header(path, header_offset, old_header_len as u64, &new_header)
}

fn reject_structural(keyword: &str) -> Result<(), FitsError> {
    if is_structural_keyword(keyword) {
        Err(FitsError::Processing(format!(
            "{keyword} is a structural keyword and cannot be edited"
        )))
    } else {
        Ok(())
    }
}

/// Inserts `card` just before any run of trailing `COMMENT`/`HISTORY`/blank
/// cards, so added keywords land among the value cards rather than after the
/// file's closing commentary block.
fn insert_before_trailing_commentary(cards: &mut Vec<Card>, card: Card) {
    let mut at = cards.len();
    while at > 0 {
        let k = &cards[at - 1].keyword;
        if k.eq_ignore_ascii_case("COMMENT") || k.eq_ignore_ascii_case("HISTORY") || k.is_empty() {
            at -= 1;
        } else {
            break;
        }
    }
    cards.insert(at, card);
}

/// Streams `[0, header_offset)` + `new_header` + `[header_offset + old_len,
/// EOF)` into a sibling temp file, fsyncs it, and renames it over `path`.
fn rewrite_with_new_header(
    path: &Path,
    header_offset: u64,
    old_header_len: u64,
    new_header: &[u8],
) -> Result<(), FitsError> {
    let dir = path.parent().unwrap_or_else(|| Path::new("."));
    let tmp_path = dir.join(format!(
        ".{}.px-fits-tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("out.fits")
    ));

    let result = (|| -> io::Result<()> {
        let mut src = File::open(path)?;
        let mut tmp = BufWriter::new(File::create(&tmp_path)?);

        io::copy(&mut Read::by_ref(&mut src).take(header_offset), &mut tmp)?;
        tmp.write_all(new_header)?;
        src.seek(SeekFrom::Start(header_offset + old_header_len))?;
        io::copy(&mut src, &mut tmp)?;

        tmp.flush()?;
        tmp.into_inner().map_err(io::Error::other)?.sync_all()?;
        Ok(())
    })();

    match result {
        Ok(()) => {
            std::fs::rename(&tmp_path, path)?;
            Ok(())
        }
        Err(e) => {
            let _ = std::fs::remove_file(&tmp_path);
            Err(e.into())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;

    fn write_to_vec<T: Pixel>(header: &HeaderBuilder, data: &[T]) -> Vec<u8> {
        let mut w = FitsWriter::new(Cursor::new(Vec::new()));
        w.write_image(header, data).unwrap();
        w.finish().unwrap().into_inner()
    }

    #[test]
    fn primary_header_is_block_aligned_and_ordered() {
        let h = HeaderBuilder::primary_image(BitPix::I16, &[4, 3]).unwrap();
        let bytes = write_to_vec::<i16>(&h, &[0i16; 12]);
        assert_eq!(bytes.len() % BLOCK_SIZE, 0);

        let head = std::str::from_utf8(&bytes[..CARD_SIZE * 6]).unwrap();
        assert!(head.starts_with("SIMPLE  =                    T"));
        assert!(head[80..].starts_with("BITPIX  =                   16"));
        assert!(head[160..].starts_with("NAXIS   =                    2"));
        assert!(head[240..].starts_with("NAXIS1  =                    4"));
        assert!(head[320..].starts_with("NAXIS2  =                    3"));
        assert!(head[400..].starts_with("EXTEND  =                    T"));
    }

    #[test]
    fn data_is_big_endian_and_zero_padded() {
        let h = HeaderBuilder::primary_image(BitPix::I16, &[2, 1]).unwrap();
        let bytes = write_to_vec::<i16>(&h, &[0x0102i16, 0x7f00]);
        let data = &bytes[BLOCK_SIZE..];
        assert_eq!(&data[..4], &[0x01, 0x02, 0x7f, 0x00]);
        assert!(data[4..].iter().all(|&b| b == 0));
        assert_eq!(bytes.len(), BLOCK_SIZE * 2);
    }

    #[test]
    fn rejects_pixel_type_not_matching_bitpix() {
        let h = HeaderBuilder::primary_image(BitPix::I16, &[2, 2]).unwrap();
        let mut w = FitsWriter::new(Cursor::new(Vec::new()));
        assert!(w.write_image::<f32>(&h, &[0.0; 4]).is_err());
    }

    #[test]
    fn rejects_wrong_data_length() {
        let h = HeaderBuilder::primary_image(BitPix::U8, &[4, 4]).unwrap();
        let mut w = FitsWriter::new(Cursor::new(Vec::new()));
        assert!(matches!(
            w.write_image::<u8>(&h, &[0u8; 15]),
            Err(FitsError::BufferLenMismatch {
                expected: 16,
                got: 15
            })
        ));
    }

    #[test]
    fn header_builder_rejects_structural_and_bad_keywords() {
        let h = HeaderBuilder::primary_image(BitPix::U8, &[1]).unwrap();
        assert!(h.clone().card("NAXIS1", Value::Integer(9), None).is_err());
        assert!(h.clone().card("bitpix", Value::Integer(8), None).is_err());
        assert!(
            h.clone()
                .card("lowercase", Value::Integer(1), None)
                .is_err()
        );
        assert!(
            h.clone()
                .card("TOOLONGKEY", Value::Integer(1), None)
                .is_err()
        );
        assert!(
            h.card("FILTER", Value::String("Ha".into()), Some("narrowband"))
                .is_ok()
        );
    }

    #[test]
    fn extra_cards_appear_after_mandatory_block() {
        let h = HeaderBuilder::primary_image(BitPix::U8, &[2, 2])
            .unwrap()
            .set_str("FILTER", "Ha", Some("narrowband"))
            .unwrap()
            .set_f64("EXPTIME", 120.0, Some("seconds"))
            .unwrap();
        let bytes = write_to_vec::<u8>(&h, &[0u8; 4]);
        let text = std::str::from_utf8(&bytes[..BLOCK_SIZE]).unwrap();
        assert!(text.contains("FILTER  = 'Ha      '"));
        assert!(text.contains("EXPTIME =                120.0"));
        let filter_at = text.find("FILTER").unwrap();
        let naxis2_at = text.find("NAXIS2").unwrap();
        assert!(filter_at > naxis2_at);
    }

    #[test]
    fn streaming_and_bulk_write_produce_identical_bytes() {
        let h = HeaderBuilder::primary_image(BitPix::I32, &[3, 4]).unwrap();
        let data: Vec<i32> = (0..12).collect();
        let bulk = write_to_vec::<i32>(&h, &data);

        let mut w = FitsWriter::new(Cursor::new(Vec::new()));
        {
            let mut iw = w.begin_image::<i32>(&h).unwrap();
            for row in data.chunks(3) {
                iw.write_row(row).unwrap();
            }
            iw.finish().unwrap();
        }
        let streamed = w.finish().unwrap().into_inner();
        assert_eq!(bulk, streamed);
    }

    #[test]
    fn image_writer_finish_errors_if_rows_missing() {
        let h = HeaderBuilder::primary_image(BitPix::U8, &[2, 3]).unwrap();
        let mut w = FitsWriter::new(Cursor::new(Vec::new()));
        let mut iw = w.begin_image::<u8>(&h).unwrap();
        iw.write_row(&[1, 2]).unwrap();
        assert!(iw.finish().is_err());
    }
}
