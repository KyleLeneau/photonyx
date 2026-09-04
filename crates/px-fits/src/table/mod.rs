//! The shared table column model (ADR 006 P6-T1): parsing `TFIELDS`,
//! `TTYPEn`/`TFORMn`/`TUNITn`/`TNULLn`/`TSCALn`/`TZEROn`/`TDIMn`/`TBCOLn`
//! into a [`ColumnDef`] list, and the [`Cell`] value type both the
//! `BINTABLE` (§7.3) and ASCII `TABLE` (§7.2) readers produce.

pub mod ascii;
pub mod binary;

pub use ascii::AsciiTableHdu;
pub use binary::BinTableHdu;

use crate::error::FitsError;
use crate::header::Header;

/// A binary-table element type (the letter in a `TFORMn` value).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BinType {
    /// `L` — logical, 1 byte (`'T'`/`'F'`/`0`).
    Logical,
    /// `X` — bit array; the repeat count is a bit count.
    Bit,
    /// `B` — unsigned byte.
    Byte,
    /// `I` — 16-bit big-endian integer.
    I16,
    /// `J` — 32-bit big-endian integer.
    I32,
    /// `K` — 64-bit big-endian integer.
    I64,
    /// `A` — ASCII character.
    Char,
    /// `E` — 32-bit IEEE float.
    F32,
    /// `D` — 64-bit IEEE float.
    F64,
    /// `C` — single-precision complex (two `E`).
    C64,
    /// `M` — double-precision complex (two `D`).
    C128,
}

impl BinType {
    fn from_code(c: u8) -> Option<BinType> {
        Some(match c {
            b'L' => BinType::Logical,
            b'X' => BinType::Bit,
            b'B' => BinType::Byte,
            b'I' => BinType::I16,
            b'J' => BinType::I32,
            b'K' => BinType::I64,
            b'A' => BinType::Char,
            b'E' => BinType::F32,
            b'D' => BinType::F64,
            b'C' => BinType::C64,
            b'M' => BinType::C128,
            _ => return None,
        })
    }

    /// Bytes per element (`Bit` is counted per byte at the field level, so 0
    /// here).
    fn elem_bytes(self) -> usize {
        match self {
            BinType::Logical | BinType::Byte | BinType::Char => 1,
            BinType::I16 => 2,
            BinType::I32 | BinType::F32 => 4,
            BinType::I64 | BinType::F64 | BinType::C64 => 8,
            BinType::C128 => 16,
            BinType::Bit => 0,
        }
    }
}

/// A variable-length-array descriptor kind: `P` (32-bit count+offset) or `Q`
/// (64-bit).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VarKind {
    P,
    Q,
}

impl VarKind {
    /// Descriptor size in the fixed row: `P` is two `i32`, `Q` is two `i64`.
    pub fn descriptor_bytes(self) -> usize {
        match self {
            VarKind::P => 8,
            VarKind::Q => 16,
        }
    }
}

/// A parsed `TFORMn` for a binary-table column.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BinFormat {
    /// `rT` — `count` fixed elements of `ty` (for `X`, `count` is a bit count).
    Fixed { ty: BinType, count: usize },
    /// `rPt(max)` / `rQt(max)` — a variable-length array of `elem`.
    Var {
        kind: VarKind,
        elem: BinType,
        max: Option<usize>,
    },
}

impl BinFormat {
    /// Bytes this column occupies in every fixed row.
    pub fn field_bytes(&self) -> usize {
        match self {
            BinFormat::Fixed {
                ty: BinType::Bit,
                count,
            } => count.div_ceil(8),
            BinFormat::Fixed { ty, count } => ty.elem_bytes() * count,
            BinFormat::Var { kind, .. } => kind.descriptor_bytes(),
        }
    }

    fn parse(raw: &str) -> Result<BinFormat, FitsError> {
        let s = raw.trim();
        let bytes = s.as_bytes();
        let mut i = 0;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let repeat: usize = if i == 0 {
            1
        } else {
            s[..i].parse().unwrap_or(1)
        };
        let rest = &bytes[i..];
        let bad = || FitsError::Processing(format!("unsupported TFORM {raw:?}"));
        let code = *rest.first().ok_or_else(bad)?;

        if code == b'P' || code == b'Q' {
            let kind = if code == b'P' { VarKind::P } else { VarKind::Q };
            let elem = rest
                .get(1)
                .copied()
                .and_then(BinType::from_code)
                .ok_or_else(bad)?;
            // Optional `(max)`.
            let max = s
                .find('(')
                .and_then(|o| s[o + 1..].find(')').map(|c| &s[o + 1..o + 1 + c]))
                .and_then(|inner| inner.trim().parse::<usize>().ok());
            return Ok(BinFormat::Var { kind, elem, max });
        }

        let ty = BinType::from_code(code).ok_or_else(bad)?;
        Ok(BinFormat::Fixed { ty, count: repeat })
    }
}

/// A parsed ASCII-`TABLE` `TFORMn` (Fortran-style, §7.2.5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AsciiFormat {
    /// `Aw`
    Char(usize),
    /// `Iw`
    Int(usize),
    /// `Fw.d`
    Fixed { width: usize, decimals: usize },
    /// `Ew.d` / `Dw.d`
    Exp { width: usize, decimals: usize },
}

impl AsciiFormat {
    pub fn width(&self) -> usize {
        match *self {
            AsciiFormat::Char(w) | AsciiFormat::Int(w) => w,
            AsciiFormat::Fixed { width, .. } | AsciiFormat::Exp { width, .. } => width,
        }
    }

    fn parse(raw: &str) -> Result<AsciiFormat, FitsError> {
        let s = raw.trim();
        let bad = || FitsError::Processing(format!("unsupported ASCII TFORM {raw:?}"));
        let code = s.as_bytes().first().copied().ok_or_else(bad)?;
        let (w_str, d_str) = match s[1..].split_once('.') {
            Some((w, d)) => (w, Some(d)),
            None => (&s[1..], None),
        };
        let width: usize = w_str.trim().parse().map_err(|_| bad())?;
        let decimals = d_str.map(|d| d.trim().parse().unwrap_or(0)).unwrap_or(0);
        Ok(match code {
            b'A' => AsciiFormat::Char(width),
            b'I' => AsciiFormat::Int(width),
            b'F' => AsciiFormat::Fixed { width, decimals },
            b'E' | b'D' => AsciiFormat::Exp { width, decimals },
            _ => return Err(bad()),
        })
    }
}

/// One table column's full definition.
#[derive(Debug, Clone)]
pub struct ColumnDef {
    /// `TTYPEn` (may be empty — the standard allows unnamed columns).
    pub name: String,
    /// `TUNITn`.
    pub unit: Option<String>,
    /// `TSCALn`, default `1.0`.
    pub scale: f64,
    /// `TZEROn`, default `0.0`.
    pub zero: f64,
    /// `TNULLn` — the integer null value (binary tables) or the raw null
    /// string (ASCII tables), whichever the table kind uses.
    pub null_int: Option<i64>,
    pub null_str: Option<String>,
    /// `TDIMn`, parsed as an axis list (fastest-varying axis first).
    pub dim: Option<Vec<usize>>,
    pub format: ColumnFormat,
}

#[derive(Debug, Clone)]
pub enum ColumnFormat {
    Binary(BinFormat),
    /// ASCII column: 0-based start byte within the row (`TBCOLn - 1`).
    Ascii {
        start: usize,
        format: AsciiFormat,
    },
}

impl ColumnDef {
    /// `true` when scaling is not the identity, so numeric cells must come
    /// back as physical floats.
    pub fn scaled(&self) -> bool {
        self.scale != 1.0 || self.zero != 0.0
    }

    /// The integer offset for a `TSCAL == 1`, integral-`TZERO` column (the
    /// unsigned-convention fast path, ADR 006 D6). `None` means "use float
    /// scaling".
    pub fn int_zero(&self) -> Option<i64> {
        if self.scale == 1.0 && self.zero.fract() == 0.0 && self.zero.abs() < 9.007e15 {
            Some(self.zero as i64)
        } else {
            None
        }
    }
}

/// One decoded table cell.
#[derive(Debug, Clone, PartialEq)]
pub enum Cell {
    Null,
    Bool(bool),
    Int(i64),
    Float(f64),
    Str(String),
    Bools(Vec<bool>),
    Ints(Vec<i64>),
    Floats(Vec<f64>),
    /// `rB` with `r > 1`, or an `X` bit field, as raw bytes.
    Bytes(Vec<u8>),
    Complex(f64, f64),
    Complexes(Vec<(f64, f64)>),
}

impl Cell {
    pub fn as_i64(&self) -> Option<i64> {
        match self {
            Cell::Int(v) => Some(*v),
            Cell::Bool(b) => Some(*b as i64),
            Cell::Float(f) => Some(*f as i64),
            _ => None,
        }
    }

    pub fn as_f64(&self) -> Option<f64> {
        match self {
            Cell::Float(v) => Some(*v),
            Cell::Int(v) => Some(*v as f64),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            Cell::Str(s) => Some(s),
            _ => None,
        }
    }

    pub fn as_i64_vec(&self) -> Option<Vec<i64>> {
        match self {
            Cell::Ints(v) => Some(v.clone()),
            Cell::Int(v) => Some(vec![*v]),
            _ => None,
        }
    }

    pub fn as_f64_vec(&self) -> Option<Vec<f64>> {
        match self {
            Cell::Floats(v) => Some(v.clone()),
            Cell::Float(v) => Some(vec![*v]),
            Cell::Ints(v) => Some(v.iter().map(|&x| x as f64).collect()),
            _ => None,
        }
    }
}

/// Reads a keyword indexed by column number (`KEYn`).
fn indexed_string(header: &Header, key: &str, n: usize) -> Option<String> {
    header.get_string(&format!("{key}{n}"))
}
fn indexed_f64(header: &Header, key: &str, n: usize) -> Option<f64> {
    header.get_f64(&format!("{key}{n}"))
}
fn indexed_i64(header: &Header, key: &str, n: usize) -> Option<i64> {
    header.get_i64(&format!("{key}{n}"))
}

/// Parses `(d1,d2,...)` from a `TDIMn` value.
fn parse_tdim(raw: &str) -> Option<Vec<usize>> {
    let inner = raw.trim().strip_prefix('(')?.strip_suffix(')')?;
    inner
        .split(',')
        .map(|t| t.trim().parse::<usize>().ok())
        .collect()
}

/// Parses every column of a table HDU from its header. `ascii` selects the
/// `TFORMn` grammar and whether `TBCOLn` is required.
pub(crate) fn parse_columns(header: &Header, ascii: bool) -> Result<Vec<ColumnDef>, FitsError> {
    let tfields = header
        .get_i64("TFIELDS")
        .ok_or_else(|| FitsError::Processing("table HDU has no TFIELDS".to_string()))?;
    let tfields = usize::try_from(tfields)
        .map_err(|_| FitsError::Processing(format!("bad TFIELDS {tfields}")))?;

    let mut columns = Vec::with_capacity(tfields);
    for n in 1..=tfields {
        let tform = indexed_string(header, "TFORM", n)
            .ok_or_else(|| FitsError::Processing(format!("column {n} has no TFORM{n}")))?;

        let format = if ascii {
            let start = indexed_i64(header, "TBCOL", n).ok_or_else(|| {
                FitsError::Processing(format!("ASCII table column {n} has no TBCOL{n}"))
            })?;
            let start = usize::try_from(start - 1)
                .map_err(|_| FitsError::Processing(format!("bad TBCOL{n} {start}")))?;
            ColumnFormat::Ascii {
                start,
                format: AsciiFormat::parse(&tform)?,
            }
        } else {
            ColumnFormat::Binary(BinFormat::parse(&tform)?)
        };

        let (null_int, null_str) = match indexed_string(header, "TNULL", n) {
            Some(s) => (s.trim().parse::<i64>().ok(), Some(s)),
            None => (indexed_i64(header, "TNULL", n), None),
        };

        columns.push(ColumnDef {
            name: indexed_string(header, "TTYPE", n).unwrap_or_default(),
            unit: indexed_string(header, "TUNIT", n),
            scale: indexed_f64(header, "TSCAL", n).unwrap_or(1.0),
            zero: indexed_f64(header, "TZERO", n).unwrap_or(0.0),
            null_int,
            null_str,
            dim: indexed_string(header, "TDIM", n).and_then(|s| parse_tdim(&s)),
            format,
        });
    }
    Ok(columns)
}

/// Finds a column by (case-insensitive) `TTYPE` name.
pub(crate) fn column_index(columns: &[ColumnDef], name: &str) -> Option<usize> {
    columns
        .iter()
        .position(|c| c.name.eq_ignore_ascii_case(name))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parse_fixed_bin_formats() {
        assert_eq!(
            BinFormat::parse("J").unwrap(),
            BinFormat::Fixed {
                ty: BinType::I32,
                count: 1
            }
        );
        assert_eq!(
            BinFormat::parse("16A").unwrap(),
            BinFormat::Fixed {
                ty: BinType::Char,
                count: 16
            }
        );
        assert_eq!(BinFormat::parse("3D").unwrap().field_bytes(), 24);
        assert_eq!(
            BinFormat::parse("17X").unwrap().field_bytes(),
            3, // ceil(17/8)
        );
    }

    #[test]
    fn parse_variable_length_descriptors() {
        assert_eq!(
            BinFormat::parse("1PJ(52)").unwrap(),
            BinFormat::Var {
                kind: VarKind::P,
                elem: BinType::I32,
                max: Some(52)
            }
        );
        assert_eq!(BinFormat::parse("PB").unwrap().field_bytes(), 8);
        assert_eq!(BinFormat::parse("QD").unwrap().field_bytes(), 16);
    }

    #[test]
    fn parse_ascii_formats() {
        assert_eq!(AsciiFormat::parse("A8").unwrap(), AsciiFormat::Char(8));
        assert_eq!(AsciiFormat::parse("I10").unwrap(), AsciiFormat::Int(10));
        assert_eq!(
            AsciiFormat::parse("F12.4").unwrap(),
            AsciiFormat::Fixed {
                width: 12,
                decimals: 4
            }
        );
        assert_eq!(
            AsciiFormat::parse("E15.7").unwrap(),
            AsciiFormat::Exp {
                width: 15,
                decimals: 7
            }
        );
    }

    #[test]
    fn tdim_parses_axis_list() {
        assert_eq!(parse_tdim("(4,2)"), Some(vec![4, 2]));
        assert_eq!(parse_tdim(" (10, 20, 3) "), Some(vec![10, 20, 3]));
        assert_eq!(parse_tdim("garbage"), None);
    }
}
