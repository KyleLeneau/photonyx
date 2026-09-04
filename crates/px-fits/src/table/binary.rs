//! `BINTABLE` reads (FITS Standard 4.0 §7.3), including the variable-length
//! array convention (`P`/`Q` descriptors + the `PCOUNT` heap, §7.3.5).
//!
//! Row access reads a whole row in one positioned read; column access reads
//! only that column's byte range in each row (one read per row) — provable
//! with `CountingSource` (ADR 006 P6-T2 gate).

use crate::error::FitsError;
use crate::hdu::DiscoveredHdu;
use crate::header::Header;
use crate::source::ByteSource;
use crate::table::{
    BinFormat, BinType, Cell, ColumnDef, ColumnFormat, VarKind, column_index, parse_columns,
};

/// A binary-table HDU bound to its byte source.
#[derive(Debug)]
pub struct BinTableHdu<'a, S: ByteSource + ?Sized> {
    source: &'a S,
    header: Header,
    columns: Vec<ColumnDef>,
    /// Byte offsets of each column's field within a row (parallel to `columns`).
    field_offsets: Vec<usize>,
    row_bytes: usize,
    nrows: usize,
    data_offset: u64,
    heap_start: u64,
    heap_len: u64,
}

impl<'a, S: ByteSource + ?Sized> BinTableHdu<'a, S> {
    pub(crate) fn from_discovered(source: &'a S, hdu: DiscoveredHdu) -> Result<Self, FitsError> {
        let header = hdu.header;
        let columns = parse_columns(&header, false)?;

        let row_bytes = header
            .get_i64("NAXIS1")
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| FitsError::Processing("BINTABLE missing NAXIS1".to_string()))?;
        let nrows = header
            .get_i64("NAXIS2")
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| FitsError::Processing("BINTABLE missing NAXIS2".to_string()))?;

        let mut field_offsets = Vec::with_capacity(columns.len());
        let mut off = 0usize;
        for c in &columns {
            field_offsets.push(off);
            let ColumnFormat::Binary(f) = &c.format else {
                return Err(FitsError::Processing(
                    "ASCII column format in a BINTABLE".to_string(),
                ));
            };
            off += f.field_bytes();
        }
        if off > row_bytes {
            return Err(FitsError::Processing(format!(
                "BINTABLE columns total {off} bytes but NAXIS1 is {row_bytes}"
            )));
        }

        let pcount = header.get_i64("PCOUNT").unwrap_or(0).max(0) as u64;
        let default_heap = (row_bytes as u64).saturating_mul(nrows as u64);
        let theap = header
            .get_i64("THEAP")
            .and_then(|v| u64::try_from(v).ok())
            .unwrap_or(default_heap);

        Ok(Self {
            source,
            heap_start: hdu.data_offset + theap,
            heap_len: pcount,
            data_offset: hdu.data_offset,
            header,
            columns,
            field_offsets,
            row_bytes,
            nrows,
        })
    }

    pub fn header(&self) -> &Header {
        &self.header
    }

    pub fn columns(&self) -> &[ColumnDef] {
        &self.columns
    }

    pub fn nrows(&self) -> usize {
        self.nrows
    }

    pub fn row_bytes(&self) -> usize {
        self.row_bytes
    }

    fn bin_format(&self, col: usize) -> &BinFormat {
        match &self.columns[col].format {
            ColumnFormat::Binary(f) => f,
            ColumnFormat::Ascii { .. } => unreachable!("checked in from_discovered"),
        }
    }

    /// Reads row `index` and decodes every column.
    pub fn row(&self, index: usize) -> Result<Vec<Cell>, FitsError> {
        if index >= self.nrows {
            return Err(FitsError::Processing(format!(
                "row {index} out of range (nrows = {})",
                self.nrows
            )));
        }
        let mut buf = vec![0u8; self.row_bytes];
        self.source
            .read_exact_at(&mut buf, self.data_offset + (index * self.row_bytes) as u64)?;

        let mut out = Vec::with_capacity(self.columns.len());
        for col in 0..self.columns.len() {
            let start = self.field_offsets[col];
            let field = &buf[start..start + self.bin_format(col).field_bytes()];
            out.push(self.decode_field(col, field)?);
        }
        Ok(out)
    }

    /// Reads a single column across all rows, touching only that column's
    /// byte range in each row (plus the heap, for a variable-length column).
    pub fn column(&self, name: &str) -> Result<Vec<Cell>, FitsError> {
        let idx = column_index(&self.columns, name)
            .ok_or_else(|| FitsError::Processing(format!("no column named {name:?}")))?;
        self.column_at(idx)
    }

    pub fn column_at(&self, col: usize) -> Result<Vec<Cell>, FitsError> {
        if col >= self.columns.len() {
            return Err(FitsError::Processing(format!("column {col} out of range")));
        }
        let field_bytes = self.bin_format(col).field_bytes();
        let field_off = self.field_offsets[col];
        let mut buf = vec![0u8; field_bytes];
        let mut out = Vec::with_capacity(self.nrows);
        for r in 0..self.nrows {
            let at = self.data_offset + (r * self.row_bytes + field_off) as u64;
            self.source.read_exact_at(&mut buf, at)?;
            out.push(self.decode_field(col, &buf)?);
        }
        Ok(out)
    }

    /// True when column `col` is a variable-length (`P`/`Q`) column.
    pub fn is_variable_length(&self, col: usize) -> bool {
        matches!(
            self.columns.get(col).map(|c| &c.format),
            Some(ColumnFormat::Binary(BinFormat::Var { .. }))
        )
    }

    /// Raw heap bytes of a variable-length column at `row`, without any
    /// element decoding — used by the tile-compression layer, whose column
    /// payloads are opaque compressed byte streams. Touches only the
    /// descriptor field in `row` and the referenced heap span.
    pub fn var_raw_bytes(&self, col: usize, row: usize) -> Result<Vec<u8>, FitsError> {
        if row >= self.nrows {
            return Err(FitsError::Processing(format!("row {row} out of range")));
        }
        let BinFormat::Var { kind, .. } = *self.bin_format(col) else {
            return Err(FitsError::Processing(format!(
                "column {col} is not variable-length"
            )));
        };
        let desc_bytes = kind.descriptor_bytes();
        let mut field = vec![0u8; desc_bytes];
        let at = self.data_offset + (row * self.row_bytes + self.field_offsets[col]) as u64;
        self.source.read_exact_at(&mut field, at)?;

        let (nelem, offset) = match kind {
            VarKind::P => (
                i32::from_be_bytes(field[0..4].try_into().unwrap()) as i64,
                i32::from_be_bytes(field[4..8].try_into().unwrap()) as i64,
            ),
            VarKind::Q => (
                i64::from_be_bytes(field[0..8].try_into().unwrap()),
                i64::from_be_bytes(field[8..16].try_into().unwrap()),
            ),
        };
        if nelem < 0 || offset < 0 {
            return Err(FitsError::HeapOutOfBounds);
        }
        let (nelem, offset) = (nelem as u64, offset as u64);
        let end = offset
            .checked_add(nelem)
            .ok_or(FitsError::HeapOutOfBounds)?;
        if end > self.heap_len {
            return Err(FitsError::HeapOutOfBounds);
        }
        let mut bytes = vec![0u8; nelem as usize];
        if nelem > 0 {
            self.source
                .read_exact_at(&mut bytes, self.heap_start + offset)?;
        }
        Ok(bytes)
    }

    fn decode_field(&self, col: usize, field: &[u8]) -> Result<Cell, FitsError> {
        let def = &self.columns[col];
        match self.bin_format(col) {
            BinFormat::Fixed { ty, count } => Ok(decode_fixed(def, *ty, *count, field, true)),
            BinFormat::Var { kind, elem, .. } => self.decode_var(def, *kind, *elem, field),
        }
    }

    fn decode_var(
        &self,
        def: &ColumnDef,
        kind: VarKind,
        elem: BinType,
        field: &[u8],
    ) -> Result<Cell, FitsError> {
        let (nelem, offset) = match kind {
            VarKind::P => (
                i32::from_be_bytes(field[0..4].try_into().unwrap()) as i64,
                i32::from_be_bytes(field[4..8].try_into().unwrap()) as i64,
            ),
            VarKind::Q => (
                i64::from_be_bytes(field[0..8].try_into().unwrap()),
                i64::from_be_bytes(field[8..16].try_into().unwrap()),
            ),
        };
        if nelem < 0 || offset < 0 {
            return Err(FitsError::HeapOutOfBounds);
        }
        let nelem = nelem as u64;
        let offset = offset as u64;
        let elem_bytes = if elem == BinType::Bit {
            nelem.div_ceil(8)
        } else {
            nelem * elem.elem_bytes() as u64
        };
        // O6 allocation guard: the span must lie wholly within the declared heap.
        let end = offset
            .checked_add(elem_bytes)
            .ok_or(FitsError::HeapOutOfBounds)?;
        if end > self.heap_len {
            return Err(FitsError::HeapOutOfBounds);
        }

        if elem_bytes == 0 {
            // Empty array — never touch the heap.
            return Ok(decode_fixed(def, elem, 0, &[], false));
        }
        let mut bytes = vec![0u8; elem_bytes as usize];
        self.source
            .read_exact_at(&mut bytes, self.heap_start + offset)?;
        Ok(decode_fixed(def, elem, nelem as usize, &bytes, false))
    }
}

/// Decodes `count` elements of `ty` from `field`, applying `TSCAL`/`TZERO`
/// and `TNULL`. When `collapse_scalar` is set, a `count == 1` field comes
/// back as a scalar `Cell`; variable-length arrays pass `false` so a
/// one-element array stays an array.
fn decode_fixed(
    def: &ColumnDef,
    ty: BinType,
    count: usize,
    field: &[u8],
    collapse_scalar: bool,
) -> Cell {
    let scalar = collapse_scalar && count == 1;

    match ty {
        BinType::Char => {
            let s = String::from_utf8_lossy(field).trim_end().to_string();
            Cell::Str(s)
        }
        BinType::Bit => Cell::Bytes(field.to_vec()),
        BinType::Logical => {
            let bools: Vec<bool> = field.iter().map(|&b| b == b'T').collect();
            if scalar {
                Cell::Bool(bools.first().copied().unwrap_or(false))
            } else {
                Cell::Bools(bools)
            }
        }
        BinType::F32 => {
            let vals: Vec<f64> = field
                .chunks_exact(4)
                .map(|c| scale(def, f32::from_be_bytes(c.try_into().unwrap()) as f64))
                .collect();
            pack_floats(vals, scalar)
        }
        BinType::F64 => {
            let vals: Vec<f64> = field
                .chunks_exact(8)
                .map(|c| scale(def, f64::from_be_bytes(c.try_into().unwrap())))
                .collect();
            pack_floats(vals, scalar)
        }
        BinType::C64 => {
            let vals: Vec<(f64, f64)> = field
                .chunks_exact(8)
                .map(|c| {
                    (
                        f32::from_be_bytes(c[0..4].try_into().unwrap()) as f64,
                        f32::from_be_bytes(c[4..8].try_into().unwrap()) as f64,
                    )
                })
                .collect();
            pack_complex(vals, scalar)
        }
        BinType::C128 => {
            let vals: Vec<(f64, f64)> = field
                .chunks_exact(16)
                .map(|c| {
                    (
                        f64::from_be_bytes(c[0..8].try_into().unwrap()),
                        f64::from_be_bytes(c[8..16].try_into().unwrap()),
                    )
                })
                .collect();
            pack_complex(vals, scalar)
        }
        BinType::Byte => decode_ints(def, field.iter().map(|&b| b as i64), scalar),
        BinType::I16 => decode_ints(
            def,
            field
                .chunks_exact(2)
                .map(|c| i16::from_be_bytes(c.try_into().unwrap()) as i64),
            scalar,
        ),
        BinType::I32 => decode_ints(
            def,
            field
                .chunks_exact(4)
                .map(|c| i32::from_be_bytes(c.try_into().unwrap()) as i64),
            scalar,
        ),
        BinType::I64 => decode_ints(
            def,
            field
                .chunks_exact(8)
                .map(|c| i64::from_be_bytes(c.try_into().unwrap())),
            scalar,
        ),
    }
}

fn scale(def: &ColumnDef, raw: f64) -> f64 {
    if def.scaled() {
        def.zero + def.scale * raw
    } else {
        raw
    }
}

fn decode_ints(def: &ColumnDef, raws: impl Iterator<Item = i64>, scalar: bool) -> Cell {
    let null = def.null_int;
    let int_zero = def.int_zero();
    let mut ints: Vec<i64> = Vec::new();
    let mut floats: Vec<f64> = Vec::new();
    let mut any_null = false;

    for raw in raws {
        if Some(raw) == null {
            any_null = true;
            ints.push(raw);
            floats.push(f64::NAN);
        } else if let Some(z) = int_zero {
            ints.push(raw.wrapping_add(z));
            floats.push(raw as f64);
        } else {
            ints.push(raw);
            floats.push(def.zero + def.scale * raw as f64);
        }
    }

    if def.scaled() && int_zero.is_none() {
        return pack_floats(floats, scalar);
    }
    if scalar {
        if any_null {
            Cell::Null
        } else {
            Cell::Int(ints[0])
        }
    } else {
        Cell::Ints(ints)
    }
}

fn pack_floats(vals: Vec<f64>, scalar: bool) -> Cell {
    if scalar {
        Cell::Float(vals.first().copied().unwrap_or(f64::NAN))
    } else {
        Cell::Floats(vals)
    }
}

fn pack_complex(vals: Vec<(f64, f64)>, scalar: bool) -> Cell {
    if scalar {
        let (re, im) = vals.first().copied().unwrap_or((f64::NAN, f64::NAN));
        Cell::Complex(re, im)
    } else {
        Cell::Complexes(vals)
    }
}
