//! ASCII `TABLE` reads (FITS Standard 4.0 §7.2): fixed-width character
//! fields located by `TBCOLn`, parsed per a Fortran-style `TFORMn`.
//!
//! Like the binary reader, `row` reads a whole row and `column` reads only
//! the one field's bytes per row.

use crate::error::FitsError;
use crate::hdu::DiscoveredHdu;
use crate::header::Header;
use crate::source::ByteSource;
use crate::table::{AsciiFormat, Cell, ColumnDef, ColumnFormat, column_index, parse_columns};

/// An ASCII-table HDU bound to its byte source.
#[derive(Debug)]
pub struct AsciiTableHdu<'a, S: ByteSource + ?Sized> {
    source: &'a S,
    header: Header,
    columns: Vec<ColumnDef>,
    row_bytes: usize,
    nrows: usize,
    data_offset: u64,
}

impl<'a, S: ByteSource + ?Sized> AsciiTableHdu<'a, S> {
    pub(crate) fn from_discovered(source: &'a S, hdu: DiscoveredHdu) -> Result<Self, FitsError> {
        let header = hdu.header;
        let columns = parse_columns(&header, true)?;
        let row_bytes = header
            .get_i64("NAXIS1")
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| FitsError::Processing("TABLE missing NAXIS1".to_string()))?;
        let nrows = header
            .get_i64("NAXIS2")
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| FitsError::Processing("TABLE missing NAXIS2".to_string()))?;

        for c in &columns {
            let ColumnFormat::Ascii { start, format } = &c.format else {
                return Err(FitsError::Processing(
                    "binary column format in an ASCII TABLE".to_string(),
                ));
            };
            if start + format.width() > row_bytes {
                return Err(FitsError::Processing(format!(
                    "ASCII column {:?} runs past the {row_bytes}-byte row",
                    c.name
                )));
            }
        }

        Ok(Self {
            source,
            header,
            columns,
            row_bytes,
            nrows,
            data_offset: hdu.data_offset,
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

    fn ascii_field(&self, col: usize) -> (usize, AsciiFormat) {
        match &self.columns[col].format {
            ColumnFormat::Ascii { start, format } => (*start, *format),
            ColumnFormat::Binary(_) => unreachable!("checked in from_discovered"),
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
        (0..self.columns.len())
            .map(|col| {
                let (start, fmt) = self.ascii_field(col);
                Ok(decode_ascii(
                    &self.columns[col],
                    fmt,
                    &buf[start..start + fmt.width()],
                ))
            })
            .collect()
    }

    /// Reads one column across all rows, touching only that field's bytes per
    /// row.
    pub fn column(&self, name: &str) -> Result<Vec<Cell>, FitsError> {
        let idx = column_index(&self.columns, name)
            .ok_or_else(|| FitsError::Processing(format!("no column named {name:?}")))?;
        self.column_at(idx)
    }

    pub fn column_at(&self, col: usize) -> Result<Vec<Cell>, FitsError> {
        if col >= self.columns.len() {
            return Err(FitsError::Processing(format!("column {col} out of range")));
        }
        let (start, fmt) = self.ascii_field(col);
        let mut buf = vec![0u8; fmt.width()];
        let mut out = Vec::with_capacity(self.nrows);
        for r in 0..self.nrows {
            let at = self.data_offset + (r * self.row_bytes + start) as u64;
            self.source.read_exact_at(&mut buf, at)?;
            out.push(decode_ascii(&self.columns[col], fmt, &buf));
        }
        Ok(out)
    }
}

fn decode_ascii(def: &ColumnDef, fmt: AsciiFormat, raw: &[u8]) -> Cell {
    let text = String::from_utf8_lossy(raw);
    let trimmed = text.trim();

    // A field that is all blank, or that matches TNULLn, is undefined.
    let is_null = trimmed.is_empty()
        || def
            .null_str
            .as_deref()
            .map(|n| n.trim() == trimmed)
            .unwrap_or(false);

    match fmt {
        AsciiFormat::Char(_) => Cell::Str(text.trim_end().to_string()),
        AsciiFormat::Int(_) => {
            if is_null {
                return Cell::Null;
            }
            match trimmed.parse::<i64>() {
                Ok(raw) => {
                    if def.scaled() {
                        Cell::Float(def.zero + def.scale * raw as f64)
                    } else {
                        Cell::Int(raw)
                    }
                }
                Err(_) => Cell::Null,
            }
        }
        AsciiFormat::Fixed { .. } | AsciiFormat::Exp { .. } => {
            if is_null {
                return Cell::Null;
            }
            // FITS permits a Fortran `D` exponent marker.
            let normalized = trimmed.replace(['D', 'd'], "E");
            match normalized.parse::<f64>() {
                Ok(raw) => Cell::Float(if def.scaled() {
                    def.zero + def.scale * raw
                } else {
                    raw
                }),
                Err(_) => Cell::Null,
            }
        }
    }
}
