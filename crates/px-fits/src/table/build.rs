//! Table writing (ADR 006 P6-T5): [`BinTableBuilder`] and
//! [`AsciiTableBuilder`], mirroring [`crate::writer::HeaderBuilder`]'s
//! chained, fallible construction. Each builder collects a column spec plus
//! row data ([`Cell`]s) and serializes a complete extension HDU —
//! `XTENSION`/`BITPIX`/`NAXIS`/`NAXIS1`/`NAXIS2`/`PCOUNT`/`GCOUNT`/`TFIELDS`
//! and the `T*n` cards, then the row bytes (and, for a `BINTABLE`, the
//! variable-length heap) padded to a 2880-byte block.

use crate::card::Value;
use crate::error::FitsError;
use crate::table::{AsciiFormat, BinFormat, BinType, Cell, VarKind};
use crate::writer::{assemble_header, serialize_card};

const BLOCK_SIZE: usize = crate::block::BLOCK_SIZE;

fn pad_data(mut bytes: Vec<u8>) -> Vec<u8> {
    let rem = bytes.len() % BLOCK_SIZE;
    if rem != 0 {
        bytes.resize(bytes.len() + (BLOCK_SIZE - rem), 0);
    }
    bytes
}

/// A card list to fixed-format header bytes.
fn header_bytes(cards: &[(String, Value, Option<String>)]) -> Result<Vec<u8>, FitsError> {
    let mut out = Vec::with_capacity(cards.len());
    for (k, v, c) in cards {
        out.push(serialize_card(k, v, c.as_deref())?);
    }
    Ok(assemble_header(&out))
}

// ===========================================================================
// BINTABLE
// ===========================================================================

#[derive(Debug, Clone)]
struct BinColumn {
    name: String,
    format: BinFormat,
    unit: Option<String>,
    scale: Option<f64>,
    zero: Option<f64>,
    null: Option<i64>,
}

/// Builds a `BINTABLE` extension HDU.
#[derive(Debug, Clone, Default)]
pub struct BinTableBuilder {
    columns: Vec<BinColumn>,
    rows: Vec<Vec<Cell>>,
    extra: Vec<(String, Value, Option<String>)>,
}

impl BinTableBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a column with a `TFORMn` string (e.g. `"J"`, `"6A"`, `"2D"`,
    /// `"1PJ(3)"`).
    pub fn column(self, name: &str, tform: &str) -> Result<Self, FitsError> {
        self.column_full(name, tform, None, None, None, None)
    }

    #[allow(clippy::too_many_arguments)]
    pub fn column_full(
        mut self,
        name: &str,
        tform: &str,
        unit: Option<&str>,
        scale: Option<f64>,
        zero: Option<f64>,
        null: Option<i64>,
    ) -> Result<Self, FitsError> {
        if !self.rows.is_empty() {
            return Err(FitsError::Processing(
                "all columns must be declared before rows are pushed".to_string(),
            ));
        }
        self.columns.push(BinColumn {
            name: name.to_string(),
            format: BinFormat::parse(tform)?,
            unit: unit.map(str::to_string),
            scale,
            zero,
            null,
        });
        Ok(self)
    }

    /// Appends a header card after the mandatory + `T*n` block.
    pub fn card(mut self, keyword: &str, value: Value, comment: Option<&str>) -> Self {
        self.extra
            .push((keyword.to_string(), value, comment.map(str::to_string)));
        self
    }

    /// Appends one row. `cells.len()` must equal the column count.
    pub fn push_row(mut self, cells: Vec<Cell>) -> Result<Self, FitsError> {
        if cells.len() != self.columns.len() {
            return Err(FitsError::Processing(format!(
                "row has {} cells but the table has {} columns",
                cells.len(),
                self.columns.len()
            )));
        }
        self.rows.push(cells);
        Ok(self)
    }

    fn row_bytes(&self) -> usize {
        self.columns.iter().map(|c| c.format.field_bytes()).sum()
    }

    /// The complete extension HDU (header blocks + rows + heap + padding).
    pub(crate) fn serialize(&self) -> Result<Vec<u8>, FitsError> {
        let row_bytes = self.row_bytes();
        let nrows = self.rows.len();

        let mut rows = Vec::with_capacity(row_bytes * nrows);
        let mut heap = Vec::new();
        for row in &self.rows {
            for (col, cell) in self.columns.iter().zip(row) {
                encode_bin_cell(col, cell, &mut rows, &mut heap)?;
            }
        }

        let mut cards: Vec<(String, Value, Option<String>)> = vec![
            (
                "XTENSION".into(),
                Value::String("BINTABLE".into()),
                Some("binary table extension".into()),
            ),
            (
                "BITPIX".into(),
                Value::Integer(8),
                Some("bits per data value".into()),
            ),
            (
                "NAXIS".into(),
                Value::Integer(2),
                Some("2-dimensional table".into()),
            ),
            (
                "NAXIS1".into(),
                Value::Integer(row_bytes as i64),
                Some("width of table row in bytes".into()),
            ),
            (
                "NAXIS2".into(),
                Value::Integer(nrows as i64),
                Some("number of rows".into()),
            ),
            (
                "PCOUNT".into(),
                Value::Integer(heap.len() as i64),
                Some("size of heap in bytes".into()),
            ),
            (
                "GCOUNT".into(),
                Value::Integer(1),
                Some("one data group".into()),
            ),
            (
                "TFIELDS".into(),
                Value::Integer(self.columns.len() as i64),
                Some("number of columns".into()),
            ),
        ];
        for (i, c) in self.columns.iter().enumerate() {
            let n = i + 1;
            cards.push((format!("TTYPE{n}"), Value::String(c.name.clone()), None));
            cards.push((
                format!("TFORM{n}"),
                Value::String(tform_string(&c.format)),
                None,
            ));
            if let Some(u) = &c.unit {
                cards.push((format!("TUNIT{n}"), Value::String(u.clone()), None));
            }
            if let Some(s) = c.scale {
                cards.push((format!("TSCAL{n}"), Value::Float(s), None));
            }
            if let Some(z) = c.zero {
                cards.push((format!("TZERO{n}"), Value::Float(z), None));
            }
            if let Some(nul) = c.null {
                cards.push((format!("TNULL{n}"), Value::Integer(nul), None));
            }
        }
        cards.extend(self.extra.iter().cloned());

        let mut out = header_bytes(&cards)?;
        rows.extend_from_slice(&heap);
        out.extend_from_slice(&pad_data(rows));
        Ok(out)
    }
}

fn tform_string(f: &BinFormat) -> String {
    match f {
        BinFormat::Fixed { ty, count } => format!("{count}{}", bin_code(*ty)),
        BinFormat::Var { kind, elem, max } => {
            let k = if *kind == VarKind::P { 'P' } else { 'Q' };
            match max {
                Some(m) => format!("1{k}{}({m})", bin_code(*elem)),
                None => format!("1{k}{}", bin_code(*elem)),
            }
        }
    }
}

fn bin_code(ty: BinType) -> char {
    match ty {
        BinType::Logical => 'L',
        BinType::Bit => 'X',
        BinType::Byte => 'B',
        BinType::I16 => 'I',
        BinType::I32 => 'J',
        BinType::I64 => 'K',
        BinType::Char => 'A',
        BinType::F32 => 'E',
        BinType::F64 => 'D',
        BinType::C64 => 'C',
        BinType::C128 => 'M',
    }
}

/// Inverse of the read-side scaling: given a physical value, the raw stored
/// integer is `(physical - zero) / scale`, rounded.
fn raw_int(col: &BinColumn, physical: f64) -> i64 {
    let zero = col.zero.unwrap_or(0.0);
    let scale = col.scale.unwrap_or(1.0);
    ((physical - zero) / scale).round() as i64
}

fn raw_float(col: &BinColumn, physical: f64) -> f64 {
    let zero = col.zero.unwrap_or(0.0);
    let scale = col.scale.unwrap_or(1.0);
    (physical - zero) / scale
}

fn encode_bin_cell(
    col: &BinColumn,
    cell: &Cell,
    row: &mut Vec<u8>,
    heap: &mut Vec<u8>,
) -> Result<(), FitsError> {
    match &col.format {
        BinFormat::Fixed { ty, count } => encode_fixed(col, *ty, *count, cell, row),
        BinFormat::Var { kind, elem, .. } => {
            let start = row.len();
            let heap_off = heap.len();
            let nelem = encode_elems(col, *elem, cell, heap)?;
            let _ = start;
            match kind {
                VarKind::P => {
                    row.extend_from_slice(&(nelem as i32).to_be_bytes());
                    row.extend_from_slice(&(heap_off as i32).to_be_bytes());
                }
                VarKind::Q => {
                    row.extend_from_slice(&(nelem as i64).to_be_bytes());
                    row.extend_from_slice(&(heap_off as i64).to_be_bytes());
                }
            }
            Ok(())
        }
    }
}

fn ints_of(cell: &Cell) -> Vec<i64> {
    match cell {
        Cell::Int(v) => vec![*v],
        Cell::Ints(v) => v.clone(),
        Cell::Bool(b) => vec![*b as i64],
        Cell::Bools(v) => v.iter().map(|&b| b as i64).collect(),
        Cell::Float(f) => vec![*f as i64],
        Cell::Floats(v) => v.iter().map(|&f| f as i64).collect(),
        Cell::Bytes(b) => b.iter().map(|&x| x as i64).collect(),
        _ => Vec::new(),
    }
}

fn floats_of(cell: &Cell) -> Vec<f64> {
    match cell {
        Cell::Float(v) => vec![*v],
        Cell::Floats(v) => v.clone(),
        Cell::Int(v) => vec![*v as f64],
        Cell::Ints(v) => v.iter().map(|&i| i as f64).collect(),
        _ => Vec::new(),
    }
}

fn wrong_cell(name: &str, ty: char) -> FitsError {
    FitsError::Processing(format!(
        "column {name:?}: cell does not fit TFORM code {ty}"
    ))
}

fn encode_fixed(
    col: &BinColumn,
    ty: BinType,
    count: usize,
    cell: &Cell,
    out: &mut Vec<u8>,
) -> Result<(), FitsError> {
    let start = out.len();
    match ty {
        BinType::Char => {
            let s = cell.as_str().ok_or_else(|| wrong_cell(&col.name, 'A'))?;
            let mut buf = vec![b' '; count];
            let b = s.as_bytes();
            let n = b.len().min(count);
            buf[..n].copy_from_slice(&b[..n]);
            out.extend_from_slice(&buf);
        }
        BinType::Bit => {
            let bytes = match cell {
                Cell::Bytes(b) => b.clone(),
                _ => return Err(wrong_cell(&col.name, 'X')),
            };
            let need = count.div_ceil(8);
            let mut buf = vec![0u8; need];
            let n = bytes.len().min(need);
            buf[..n].copy_from_slice(&bytes[..n]);
            out.extend_from_slice(&buf);
        }
        BinType::Logical => {
            let bools: Vec<bool> = match cell {
                Cell::Bool(b) => vec![*b],
                Cell::Bools(v) => v.clone(),
                _ => return Err(wrong_cell(&col.name, 'L')),
            };
            for i in 0..count {
                out.push(if *bools.get(i).unwrap_or(&false) {
                    b'T'
                } else {
                    b'F'
                });
            }
        }
        BinType::F32 => {
            let vals = floats_of(cell);
            for i in 0..count {
                let v = raw_float(col, *vals.get(i).unwrap_or(&0.0)) as f32;
                out.extend_from_slice(&v.to_be_bytes());
            }
        }
        BinType::F64 => {
            let vals = floats_of(cell);
            for i in 0..count {
                let v = raw_float(col, *vals.get(i).unwrap_or(&0.0));
                out.extend_from_slice(&v.to_be_bytes());
            }
        }
        BinType::C64 => {
            let vals = complexes_of(cell);
            for i in 0..count {
                let (re, im) = vals.get(i).copied().unwrap_or((0.0, 0.0));
                out.extend_from_slice(&(re as f32).to_be_bytes());
                out.extend_from_slice(&(im as f32).to_be_bytes());
            }
        }
        BinType::C128 => {
            let vals = complexes_of(cell);
            for i in 0..count {
                let (re, im) = vals.get(i).copied().unwrap_or((0.0, 0.0));
                out.extend_from_slice(&re.to_be_bytes());
                out.extend_from_slice(&im.to_be_bytes());
            }
        }
        BinType::Byte | BinType::I16 | BinType::I32 | BinType::I64 => {
            let raws: Vec<i64> = if matches!(cell, Cell::Null) {
                vec![col.null.unwrap_or(0); count]
            } else {
                let phys = ints_of(cell);
                (0..count)
                    .map(|i| {
                        let p = *phys.get(i).unwrap_or(&0);
                        if col.scale.is_some() || col.zero.is_some() {
                            raw_int(col, p as f64)
                        } else {
                            p
                        }
                    })
                    .collect()
            };
            for r in raws {
                match ty {
                    BinType::Byte => out.push(r as u8),
                    BinType::I16 => out.extend_from_slice(&(r as i16).to_be_bytes()),
                    BinType::I32 => out.extend_from_slice(&(r as i32).to_be_bytes()),
                    BinType::I64 => out.extend_from_slice(&r.to_be_bytes()),
                    _ => unreachable!(),
                }
            }
        }
    }
    debug_assert_eq!(
        out.len() - start,
        BinFormat::Fixed { ty, count }.field_bytes()
    );
    Ok(())
}

fn complexes_of(cell: &Cell) -> Vec<(f64, f64)> {
    match cell {
        Cell::Complex(re, im) => vec![(*re, *im)],
        Cell::Complexes(v) => v.clone(),
        _ => Vec::new(),
    }
}

/// Encodes a variable-length payload into the heap, returning the element
/// count written.
fn encode_elems(
    col: &BinColumn,
    elem: BinType,
    cell: &Cell,
    heap: &mut Vec<u8>,
) -> Result<usize, FitsError> {
    Ok(match elem {
        BinType::Char => {
            let s = cell.as_str().ok_or_else(|| wrong_cell(&col.name, 'A'))?;
            heap.extend_from_slice(s.as_bytes());
            s.len()
        }
        BinType::Byte => {
            let v = ints_of(cell);
            heap.extend(v.iter().map(|&x| x as u8));
            v.len()
        }
        BinType::I16 => {
            let v = ints_of(cell);
            for x in &v {
                heap.extend_from_slice(&(*x as i16).to_be_bytes());
            }
            v.len()
        }
        BinType::I32 => {
            let v = ints_of(cell);
            for x in &v {
                heap.extend_from_slice(&(*x as i32).to_be_bytes());
            }
            v.len()
        }
        BinType::I64 => {
            let v = ints_of(cell);
            for x in &v {
                heap.extend_from_slice(&x.to_be_bytes());
            }
            v.len()
        }
        BinType::F32 => {
            let v = floats_of(cell);
            for x in &v {
                heap.extend_from_slice(&(*x as f32).to_be_bytes());
            }
            v.len()
        }
        BinType::F64 => {
            let v = floats_of(cell);
            for x in &v {
                heap.extend_from_slice(&x.to_be_bytes());
            }
            v.len()
        }
        BinType::Logical => {
            let v = match cell {
                Cell::Bools(v) => v.clone(),
                Cell::Bool(b) => vec![*b],
                _ => return Err(wrong_cell(&col.name, 'L')),
            };
            heap.extend(v.iter().map(|&b| if b { b'T' } else { b'F' }));
            v.len()
        }
        BinType::Bit | BinType::C64 | BinType::C128 => {
            return Err(FitsError::Processing(format!(
                "column {:?}: variable-length {elem:?} elements are not supported",
                col.name
            )));
        }
    })
}

// ===========================================================================
// ASCII TABLE
// ===========================================================================

#[derive(Debug, Clone)]
struct AsciiColumn {
    name: String,
    format: AsciiFormat,
    unit: Option<String>,
}

/// Builds an ASCII `TABLE` extension HDU. Fields are laid out left to right
/// with a single space between them.
#[derive(Debug, Clone, Default)]
pub struct AsciiTableBuilder {
    columns: Vec<AsciiColumn>,
    rows: Vec<Vec<Cell>>,
    extra: Vec<(String, Value, Option<String>)>,
}

impl AsciiTableBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    /// Adds a column with a Fortran `TFORMn` (`"A10"`, `"I5"`, `"F8.3"`,
    /// `"E15.7"`).
    pub fn column(mut self, name: &str, tform: &str) -> Result<Self, FitsError> {
        if !self.rows.is_empty() {
            return Err(FitsError::Processing(
                "all columns must be declared before rows are pushed".to_string(),
            ));
        }
        self.columns.push(AsciiColumn {
            name: name.to_string(),
            format: AsciiFormat::parse(tform)?,
            unit: None,
        });
        Ok(self)
    }

    pub fn column_with_unit(self, name: &str, tform: &str, unit: &str) -> Result<Self, FitsError> {
        let mut b = self.column(name, tform)?;
        b.columns.last_mut().unwrap().unit = Some(unit.to_string());
        Ok(b)
    }

    pub fn card(mut self, keyword: &str, value: Value, comment: Option<&str>) -> Self {
        self.extra
            .push((keyword.to_string(), value, comment.map(str::to_string)));
        self
    }

    pub fn push_row(mut self, cells: Vec<Cell>) -> Result<Self, FitsError> {
        if cells.len() != self.columns.len() {
            return Err(FitsError::Processing(format!(
                "row has {} cells but the table has {} columns",
                cells.len(),
                self.columns.len()
            )));
        }
        self.rows.push(cells);
        Ok(self)
    }

    /// 0-based start byte of each column and the total row width.
    fn layout(&self) -> (Vec<usize>, usize) {
        let mut starts = Vec::with_capacity(self.columns.len());
        let mut at = 0usize;
        for (i, c) in self.columns.iter().enumerate() {
            if i > 0 {
                at += 1; // one-space separator
            }
            starts.push(at);
            at += c.format.width();
        }
        (starts, at)
    }

    pub(crate) fn serialize(&self) -> Result<Vec<u8>, FitsError> {
        let (starts, row_width) = self.layout();
        let nrows = self.rows.len();

        let mut data = Vec::with_capacity(row_width * nrows);
        for row in &self.rows {
            let mut line = vec![b' '; row_width];
            for ((col, cell), &start) in self.columns.iter().zip(row).zip(&starts) {
                let field = format_ascii_cell(&col.name, col.format, cell)?;
                let w = col.format.width();
                line[start..start + w].copy_from_slice(field.as_bytes());
            }
            data.extend_from_slice(&line);
        }

        let mut cards: Vec<(String, Value, Option<String>)> = vec![
            (
                "XTENSION".into(),
                Value::String("TABLE".into()),
                Some("ASCII table extension".into()),
            ),
            (
                "BITPIX".into(),
                Value::Integer(8),
                Some("bits per data value".into()),
            ),
            (
                "NAXIS".into(),
                Value::Integer(2),
                Some("2-dimensional table".into()),
            ),
            (
                "NAXIS1".into(),
                Value::Integer(row_width as i64),
                Some("width of table row in bytes".into()),
            ),
            (
                "NAXIS2".into(),
                Value::Integer(nrows as i64),
                Some("number of rows".into()),
            ),
            ("PCOUNT".into(), Value::Integer(0), Some("no heap".into())),
            (
                "GCOUNT".into(),
                Value::Integer(1),
                Some("one data group".into()),
            ),
            (
                "TFIELDS".into(),
                Value::Integer(self.columns.len() as i64),
                Some("number of columns".into()),
            ),
        ];
        for (i, c) in self.columns.iter().enumerate() {
            let n = i + 1;
            cards.push((format!("TTYPE{n}"), Value::String(c.name.clone()), None));
            cards.push((
                format!("TBCOL{n}"),
                Value::Integer(starts[i] as i64 + 1),
                None,
            ));
            cards.push((
                format!("TFORM{n}"),
                Value::String(ascii_tform(c.format)),
                None,
            ));
            if let Some(u) = &c.unit {
                cards.push((format!("TUNIT{n}"), Value::String(u.clone()), None));
            }
        }
        cards.extend(self.extra.iter().cloned());

        let mut out = header_bytes(&cards)?;
        out.extend_from_slice(&pad_data(data));
        Ok(out)
    }
}

fn ascii_tform(f: AsciiFormat) -> String {
    match f {
        AsciiFormat::Char(w) => format!("A{w}"),
        AsciiFormat::Int(w) => format!("I{w}"),
        AsciiFormat::Fixed { width, decimals } => format!("F{width}.{decimals}"),
        AsciiFormat::Exp { width, decimals } => format!("E{width}.{decimals}"),
    }
}

fn format_ascii_cell(name: &str, fmt: AsciiFormat, cell: &Cell) -> Result<String, FitsError> {
    let w = fmt.width();
    let overflow =
        || FitsError::Processing(format!("column {name:?}: value does not fit width {w}"));
    let field = match fmt {
        AsciiFormat::Char(_) => {
            let s = cell.as_str().unwrap_or("");
            let mut s = s.to_string();
            s.truncate(w);
            format!("{s:<w$}")
        }
        AsciiFormat::Int(_) => match cell {
            Cell::Null => " ".repeat(w),
            _ => {
                let v = cell.as_i64().ok_or_else(|| wrong_cell(name, 'I'))?;
                format!("{v:>w$}")
            }
        },
        AsciiFormat::Fixed { width, decimals } => match cell {
            Cell::Null => " ".repeat(width),
            _ => {
                let v = cell.as_f64().ok_or_else(|| wrong_cell(name, 'F'))?;
                format!("{v:>width$.decimals$}")
            }
        },
        AsciiFormat::Exp { width, decimals } => match cell {
            Cell::Null => " ".repeat(width),
            _ => {
                let v = cell.as_f64().ok_or_else(|| wrong_cell(name, 'E'))?;
                format!("{v:>width$.decimals$E}")
            }
        },
    };
    if field.len() != w {
        return Err(overflow());
    }
    Ok(field)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::reader::FitsReader;
    use crate::source::SliceSource;

    /// Prepends a minimal empty primary so the bytes are a valid FITS file.
    fn with_primary(ext: Vec<u8>) -> Vec<u8> {
        let primary =
            crate::writer::HeaderBuilder::primary_image(crate::header::BitPix::U8, &[]).unwrap();
        let mut w = crate::writer::FitsWriter::new(Vec::new());
        w.write_image::<u8>(&primary, &[]).unwrap();
        let mut bytes = w.finish().unwrap();
        bytes.extend_from_slice(&ext);
        bytes
    }

    #[test]
    fn bintable_builder_roundtrips_through_the_reader() {
        let ext = BinTableBuilder::new()
            .column("ID", "J")
            .unwrap()
            .column("NAME", "5A")
            .unwrap()
            .column_full("CNT", "I", None, Some(1.0), Some(32768.0), None)
            .unwrap()
            .column("SAMP", "1PJ(4)")
            .unwrap()
            .push_row(vec![
                Cell::Int(7),
                Cell::Str("abc".into()),
                Cell::Int(40000),
                Cell::Ints(vec![1, 2, 3]),
            ])
            .unwrap()
            .push_row(vec![
                Cell::Int(9),
                Cell::Str("xy".into()),
                Cell::Int(65535),
                Cell::Ints(vec![]),
            ])
            .unwrap()
            .serialize()
            .unwrap();

        let reader = FitsReader::from_source(SliceSource::new(with_primary(ext))).unwrap();
        let t = reader.bintable(1).unwrap();
        assert_eq!(t.nrows(), 2);
        assert_eq!(t.column("ID").unwrap(), vec![Cell::Int(7), Cell::Int(9)]);
        assert_eq!(
            t.column("NAME").unwrap(),
            vec![Cell::Str("abc".into()), Cell::Str("xy".into())]
        );
        assert_eq!(
            t.column("CNT").unwrap(),
            vec![Cell::Int(40000), Cell::Int(65535)]
        );
        assert_eq!(
            t.column("SAMP").unwrap(),
            vec![Cell::Ints(vec![1, 2, 3]), Cell::Ints(vec![])]
        );
    }

    #[test]
    fn ascii_table_builder_roundtrips_through_the_reader() {
        let ext = AsciiTableBuilder::new()
            .column("SEQ", "I4")
            .unwrap()
            .column("MAG", "F8.3")
            .unwrap()
            .column("NAME", "A6")
            .unwrap()
            .push_row(vec![
                Cell::Int(1),
                Cell::Float(1.25),
                Cell::Str("Ha".into()),
            ])
            .unwrap()
            .push_row(vec![Cell::Int(2), Cell::Null, Cell::Str("OIII".into())])
            .unwrap()
            .serialize()
            .unwrap();

        let reader = FitsReader::from_source(SliceSource::new(with_primary(ext))).unwrap();
        let t = reader.ascii_table(1).unwrap();
        assert_eq!(t.column("SEQ").unwrap(), vec![Cell::Int(1), Cell::Int(2)]);
        assert_eq!(
            t.column("MAG").unwrap(),
            vec![Cell::Float(1.25), Cell::Null]
        );
        assert_eq!(
            t.column("NAME").unwrap(),
            vec![Cell::Str("Ha".into()), Cell::Str("OIII".into())]
        );
    }

    #[test]
    fn push_row_rejects_wrong_arity() {
        let b = BinTableBuilder::new().column("A", "J").unwrap();
        assert!(b.push_row(vec![Cell::Int(1), Cell::Int(2)]).is_err());
    }
}
