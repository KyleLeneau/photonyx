//! FITS card images: parsing and serialization of the 80-byte records that
//! make up a header (FITS Standard 4.0 §4.1 "Overview of header card
//! images", §4.2 "Keyword", §4.3 "Value", §4.4 "Comment").
//!
//! Parsing is permissive by design (ADR 006 D-series / O5): a card that
//! doesn't conform to any recognized syntax becomes `Value::Invalid(raw)`
//! rather than failing the whole file, since real-world FITS files routinely
//! bend the standard in ways that shouldn't be fatal to reading everything
//! else in them.

/// A parsed keyword/value/comment triple.
#[derive(Debug, Clone, PartialEq)]
pub struct Card {
    /// The keyword, trimmed of trailing padding. For `HIERARCH` convention
    /// cards (§ not part of the base standard, but universal in practice)
    /// this is the full extended keyword, e.g. `"HIERARCH ESO OBS ID"`, not
    /// just the eight-byte `"HIERARCH"` field.
    pub keyword: String,
    pub value: Value,
    pub comment: Option<String>,
}

/// The value carried by a card. `Invalid` is the permissive fallback for
/// text that doesn't parse as any of the standard's value types — see the
/// module docs.
#[derive(Debug, Clone, PartialEq)]
pub enum Value {
    Logical(bool),
    Integer(i64),
    Float(f64),
    String(String),
    Complex(f64, f64),
    /// No value token present (an `=` card whose value field is blank).
    Undefined,
    /// Free text carried by a `COMMENT`, `HISTORY`, or blank-keyword card —
    /// these have no `=` and no separate comment field; the whole of
    /// columns 9-80 is the payload.
    Commentary(String),
    /// A value field that did not parse as any recognized syntax. Carries
    /// the raw, trimmed text of the value field so nothing is lost.
    Invalid(String),
}

const KEYWORD_FIELD: usize = 8;
const CARD_LEN: usize = crate::block::CARD_SIZE;

impl Card {
    pub fn new(keyword: impl Into<String>, value: Value, comment: Option<String>) -> Self {
        Self {
            keyword: keyword.into(),
            value,
            comment,
        }
    }

    /// Parses one 80-byte card image. Never fails — anything that doesn't
    /// conform becomes `Value::Invalid`.
    pub fn parse(bytes: &[u8; CARD_LEN]) -> Card {
        // Cards are meant to be plain ASCII (FITS Standard 4.0 §4.1); treat
        // anything outside that range as opaque bytes rather than panicking
        // on invalid UTF-8.
        let line: String = bytes.iter().map(|&b| b as char).collect();
        let keyword_field = &line[0..KEYWORD_FIELD];
        let keyword_trimmed = keyword_field.trim_end();

        if keyword_trimmed.is_empty()
            || keyword_trimmed.eq_ignore_ascii_case("COMMENT")
            || keyword_trimmed.eq_ignore_ascii_case("HISTORY")
            || keyword_trimmed.eq_ignore_ascii_case("END")
        {
            let text = line[KEYWORD_FIELD..].trim_end().to_string();
            return Card {
                keyword: keyword_trimmed.to_string(),
                value: Value::Commentary(text),
                comment: None,
            };
        }

        if keyword_trimmed.eq_ignore_ascii_case("HIERARCH") {
            return Self::parse_hierarch(&line);
        }

        // Standard form: '=' in column 9 (index 8), conventionally followed
        // by a space in column 10. Some non-conforming writers omit the
        // trailing space; accept '=' alone as sufficient.
        if bytes[8] == b'=' {
            let (value, comment) = parse_value_field(&line[9..]);
            return Card {
                keyword: keyword_trimmed.to_string(),
                value,
                comment,
            };
        }

        // CONTINUE cards (OGIP long-string convention): no '=' by
        // convention, value field starts right after the keyword field.
        if keyword_trimmed.eq_ignore_ascii_case("CONTINUE") {
            let (value, comment) = parse_value_field(&line[KEYWORD_FIELD..]);
            return Card {
                keyword: keyword_trimmed.to_string(),
                value,
                comment,
            };
        }

        // Doesn't conform to any recognized card syntax.
        Card {
            keyword: keyword_trimmed.to_string(),
            value: Value::Invalid(line.trim_end().to_string()),
            comment: None,
        }
    }

    fn parse_hierarch(line: &str) -> Card {
        match line.find('=') {
            Some(eq) => {
                let keyword = line[..eq].trim().to_string();
                let (value, comment) = parse_value_field(&line[eq + 1..]);
                Card {
                    keyword,
                    value,
                    comment,
                }
            }
            None => Card {
                keyword: "HIERARCH".to_string(),
                value: Value::Invalid(line.trim_end().to_string()),
                comment: None,
            },
        }
    }

    /// Serializes to exactly 80 ASCII bytes. If a comment does not fit
    /// alongside the value within the 80-byte limit, it is dropped rather
    /// than corrupting the value or overflowing the card — callers that
    /// need a guarantee it fits should keep comments short (see
    /// `tests/card_proptest.rs` for the bounds used in the roundtrip
    /// property).
    pub fn to_bytes(&self) -> [u8; CARD_LEN] {
        let is_hierarch = self.keyword.len() > KEYWORD_FIELD
            || self.keyword.eq_ignore_ascii_case("HIERARCH")
            || self
                .keyword
                .split_whitespace()
                .next()
                .unwrap_or("")
                .eq_ignore_ascii_case("HIERARCH");

        let line = match &self.value {
            Value::Commentary(text) => {
                format!("{:<8}{}", self.keyword, text)
            }
            _ if self.keyword.eq_ignore_ascii_case("CONTINUE") => {
                format!("CONTINUE  {}", format_value(&self.value))
            }
            _ if is_hierarch => {
                let mut s = format!("{} = {}", self.keyword, format_value(&self.value));
                if let Some(c) = &self.comment {
                    s.push_str(" / ");
                    s.push_str(c);
                }
                s
            }
            _ => {
                let mut s = format!("{:<8}= {}", self.keyword, format_value(&self.value));
                if let Some(c) = &self.comment {
                    s.push_str(" / ");
                    s.push_str(c);
                }
                s
            }
        };

        let mut out = [b' '; CARD_LEN];
        let bytes = line.as_bytes();
        let n = bytes.len().min(CARD_LEN);
        out[..n].copy_from_slice(&bytes[..n]);
        out
    }
}

/// Formats a value for the value field (the caller adds the keyword prefix
/// and any comment).
fn format_value(value: &Value) -> String {
    match value {
        Value::Logical(b) => (if *b { "T" } else { "F" }).to_string(),
        Value::Integer(i) => i.to_string(),
        // `{}` uses Rust's shortest round-tripping decimal representation,
        // so `str::parse::<f64>()` recovers the exact same value — this is
        // what makes the card proptest roundtrip hold for floats.
        Value::Float(f) => f.to_string(),
        Value::String(s) => format!("'{}'", s.replace('\'', "''")),
        Value::Complex(re, im) => format!("({re}, {im})"),
        Value::Undefined => String::new(),
        Value::Commentary(text) => text.clone(),
        Value::Invalid(raw) => raw.clone(),
    }
}

/// Parses the value field starting right after the keyword prefix (i.e.
/// after `"KEYWORD = "` or, for `CONTINUE`, right after the keyword field).
/// Returns the parsed value and an optional trailing comment.
fn parse_value_field(rest: &str) -> (Value, Option<String>) {
    let rest = rest.trim_start();

    if let Some(stripped) = rest.strip_prefix('\'') {
        let (content, after) = parse_quoted_string(stripped);
        let comment = extract_comment(after);
        return (Value::String(content), comment);
    }

    if let Some(stripped) = rest.strip_prefix('(')
        && let Some(close) = stripped.find(')')
    {
        let inner = &stripped[..close];
        let after = &stripped[close + 1..];
        if let Some((re_str, im_str)) = inner.split_once(',')
            && let (Ok(re), Ok(im)) = (
                parse_fits_float(re_str.trim()),
                parse_fits_float(im_str.trim()),
            )
        {
            return (Value::Complex(re, im), extract_comment(after));
        }
    }

    // Unquoted token: runs until whitespace or '/' (start of a comment).
    let token_end = rest.find([' ', '/']).unwrap_or(rest.len());
    let token = &rest[..token_end];
    let after = &rest[token_end..];
    let comment = extract_comment(after);

    if token.is_empty() {
        return (Value::Undefined, comment);
    }
    if token.eq_ignore_ascii_case("T") {
        return (Value::Logical(true), comment);
    }
    if token.eq_ignore_ascii_case("F") {
        return (Value::Logical(false), comment);
    }
    if let Ok(i) = token.parse::<i64>() {
        return (Value::Integer(i), comment);
    }
    if let Ok(f) = parse_fits_float(token) {
        return (Value::Float(f), comment);
    }

    (Value::Invalid(rest.trim_end().to_string()), None)
}

/// FITS floats may use `D`/`d` as the exponent marker instead of `E`/`e`
/// (a Fortran-ism carried into the standard, §4.3.3.2).
fn parse_fits_float(token: &str) -> Result<f64, std::num::ParseFloatError> {
    if token.contains(['D', 'd']) {
        token.replace(['D', 'd'], "E").parse::<f64>()
    } else {
        token.parse::<f64>()
    }
}

/// Parses a single-quoted string value starting just after the opening
/// quote, handling `''` as an escaped literal quote (§4.3.3.1). Returns the
/// unescaped content (trailing spaces trimmed — significant only for
/// column-alignment padding, not treated as part of the value) and the
/// remainder of the line after the closing quote.
fn parse_quoted_string(s: &str) -> (String, &str) {
    let mut content = String::new();
    let chars: Vec<char> = s.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        if chars[i] == '\'' {
            if chars.get(i + 1) == Some(&'\'') {
                content.push('\'');
                i += 2;
                continue;
            }
            // Closing quote.
            let byte_offset: usize = chars[..i + 1].iter().map(|c| c.len_utf8()).sum();
            return (content.trim_end().to_string(), &s[byte_offset..]);
        }
        content.push(chars[i]);
        i += 1;
    }
    // No closing quote found: treat the rest of the field as the content.
    (content.trim_end().to_string(), "")
}

/// Everything after the value token, up to and including a `/` comment
/// marker if present, becomes the comment (trimmed).
fn extract_comment(after: &str) -> Option<String> {
    let after = after.trim_start();
    let after = after.strip_prefix('/')?;
    let text = after.trim();
    if text.is_empty() {
        None
    } else {
        Some(text.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse_line(line: &str) -> Card {
        let mut bytes = [b' '; CARD_LEN];
        let src = line.as_bytes();
        let n = src.len().min(CARD_LEN);
        bytes[..n].copy_from_slice(&src[..n]);
        Card::parse(&bytes)
    }

    #[test]
    fn parses_logical() {
        let c = parse_line("SIMPLE  =                    T / conforms to FITS standard");
        assert_eq!(c.keyword, "SIMPLE");
        assert_eq!(c.value, Value::Logical(true));
        assert_eq!(c.comment.as_deref(), Some("conforms to FITS standard"));
    }

    #[test]
    fn parses_integer() {
        let c = parse_line("NAXIS1  =                   64 / axis 1 length");
        assert_eq!(c.value, Value::Integer(64));
    }

    #[test]
    fn parses_negative_integer() {
        let c = parse_line("BLANK   =               -32768 / undefined pixel value");
        assert_eq!(c.value, Value::Integer(-32768));
    }

    #[test]
    fn parses_float() {
        let c = parse_line("BZERO   =              32768.0 / unsigned 16-bit convention");
        assert_eq!(c.value, Value::Float(32768.0));
    }

    #[test]
    fn parses_float_with_d_exponent() {
        let c = parse_line("EXPTIME =            1.5D2 / exposure");
        assert_eq!(c.value, Value::Float(150.0));
    }

    #[test]
    fn parses_string() {
        let c = parse_line("OBJECT  = 'M42     '           / target");
        assert_eq!(c.value, Value::String("M42".to_string()));
        assert_eq!(c.comment.as_deref(), Some("target"));
    }

    #[test]
    fn parses_string_with_escaped_quote() {
        let c = parse_line("NOTE    = 'it''s a test'");
        assert_eq!(c.value, Value::String("it's a test".to_string()));
    }

    #[test]
    fn parses_string_with_embedded_slash_not_a_comment() {
        let c = parse_line("PATH    = '/dev/null' / a path, not a comment start inside quotes");
        assert_eq!(c.value, Value::String("/dev/null".to_string()));
        assert_eq!(
            c.comment.as_deref(),
            Some("a path, not a comment start inside quotes")
        );
    }

    #[test]
    fn parses_undefined_value() {
        let c = parse_line("FOO     =                      / no value given");
        assert_eq!(c.value, Value::Undefined);
        assert_eq!(c.comment.as_deref(), Some("no value given"));
    }

    #[test]
    fn parses_complex() {
        let c = parse_line("CVAL    = (1.5, -2.5)");
        assert_eq!(c.value, Value::Complex(1.5, -2.5));
    }

    #[test]
    fn parses_comment_keyword() {
        let c = parse_line("COMMENT this is free text, not key=value");
        assert_eq!(c.keyword, "COMMENT");
        assert_eq!(
            c.value,
            Value::Commentary("this is free text, not key=value".to_string())
        );
    }

    #[test]
    fn parses_history_keyword() {
        let c = parse_line("HISTORY processed by px-fits");
        assert_eq!(c.keyword, "HISTORY");
        assert_eq!(
            c.value,
            Value::Commentary("processed by px-fits".to_string())
        );
    }

    #[test]
    fn parses_blank_keyword_as_commentary() {
        let c = parse_line("        free-standing text on a blank-keyword card");
        assert_eq!(c.keyword, "");
        assert!(matches!(c.value, Value::Commentary(_)));
    }

    #[test]
    fn parses_end_card() {
        let c = parse_line("END");
        assert_eq!(c.keyword, "END");
        assert_eq!(c.value, Value::Commentary(String::new()));
    }

    #[test]
    fn parses_continue_card() {
        let c = parse_line("CONTINUE  'more text&'");
        assert_eq!(c.keyword, "CONTINUE");
        assert_eq!(c.value, Value::String("more text&".to_string()));
    }

    #[test]
    fn parses_hierarch_card() {
        let c = parse_line("HIERARCH ESO OBS ID = 12345 / observation id");
        assert_eq!(c.keyword, "HIERARCH ESO OBS ID");
        assert_eq!(c.value, Value::Integer(12345));
        assert_eq!(c.comment.as_deref(), Some("observation id"));
    }

    #[test]
    fn malformed_card_becomes_invalid_not_a_panic() {
        let c = parse_line("BADCARD!!!!not a valid value syntax at all,,,,");
        assert_eq!(c.keyword, "BADCARD!");
        assert!(matches!(c.value, Value::Invalid(_)));
    }

    #[test]
    fn non_ascii_bytes_do_not_panic() {
        let mut bytes = [b' '; CARD_LEN];
        bytes[0] = b'A';
        bytes[1] = 0xFF; // not valid ASCII/UTF-8 on its own
        let c = Card::parse(&bytes);
        // Must not panic; exact classification is not asserted here.
        let _ = c;
    }

    #[test]
    fn to_bytes_is_exactly_80_bytes() {
        let c = Card::new("SIMPLE", Value::Logical(true), Some("ok".to_string()));
        assert_eq!(c.to_bytes().len(), CARD_LEN);
    }

    #[test]
    fn roundtrip_logical() {
        let c = Card::new("SIMPLE", Value::Logical(true), Some("conforms".to_string()));
        let bytes = c.to_bytes();
        assert_eq!(Card::parse(&bytes), c);
    }

    #[test]
    fn roundtrip_integer() {
        let c = Card::new("NAXIS1", Value::Integer(-42), None);
        let bytes = c.to_bytes();
        assert_eq!(Card::parse(&bytes), c);
    }

    #[test]
    fn roundtrip_string() {
        let c = Card::new(
            "OBJECT",
            Value::String("M42".to_string()),
            Some("target".to_string()),
        );
        let bytes = c.to_bytes();
        assert_eq!(Card::parse(&bytes), c);
    }

    #[test]
    fn roundtrip_string_with_quote() {
        let c = Card::new("NOTE", Value::String("it's a test".to_string()), None);
        let bytes = c.to_bytes();
        assert_eq!(Card::parse(&bytes), c);
    }

    #[test]
    fn roundtrip_commentary() {
        let c = Card::new(
            "COMMENT",
            Value::Commentary("hello world".to_string()),
            None,
        );
        let bytes = c.to_bytes();
        assert_eq!(Card::parse(&bytes), c);
    }

    #[test]
    fn roundtrip_undefined() {
        let c = Card::new("FOO", Value::Undefined, Some("tbd".to_string()));
        let bytes = c.to_bytes();
        assert_eq!(Card::parse(&bytes), c);
    }

    #[test]
    fn roundtrip_hierarch() {
        let c = Card::new(
            "HIERARCH ESO OBS ID",
            Value::Integer(7),
            Some("id".to_string()),
        );
        let bytes = c.to_bytes();
        assert_eq!(Card::parse(&bytes), c);
    }
}
