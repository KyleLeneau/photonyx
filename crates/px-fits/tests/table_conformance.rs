//! ADR 006 Phase 6 gate: read the committed table fixtures, and prove
//! column access touches only that column's bytes (`CountingSource`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};

use px_fits::reader::FitsReader;
use px_fits::source::{ByteSource, FileSource};
use px_fits::table::Cell;

fn fixtures_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

struct CountingSource<S> {
    inner: S,
    bytes: AtomicU64,
    calls: AtomicU64,
}
impl<S: ByteSource> CountingSource<S> {
    fn new(inner: S) -> Self {
        Self {
            inner,
            bytes: AtomicU64::new(0),
            calls: AtomicU64::new(0),
        }
    }
}
impl<S: ByteSource> ByteSource for CountingSource<S> {
    fn len(&self) -> u64 {
        self.inner.len()
    }
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
        self.inner.read_exact_at(buf, offset)?;
        self.bytes.fetch_add(buf.len() as u64, Ordering::Relaxed);
        self.calls.fetch_add(1, Ordering::Relaxed);
        Ok(())
    }
}

#[test]
fn bintable_columns_decode_to_expected_values() {
    let reader = FitsReader::open(fixtures_dir().join("bintable.fits")).unwrap();
    let table = reader.bintable(1).unwrap();
    assert_eq!(table.nrows(), 4);
    assert_eq!(table.columns().len(), 6);
    assert_eq!(table.row_bytes(), 40);

    assert_eq!(
        table.column("ID").unwrap(),
        vec![Cell::Int(1), Cell::Int(2), Cell::Int(3), Cell::Int(4)]
    );
    assert_eq!(
        table.column("FLUX").unwrap(),
        vec![
            Cell::Float(1.5),
            Cell::Float(2.5),
            Cell::Float(3.5),
            Cell::Float(4.5)
        ]
    );
    assert_eq!(
        table.column("NAME").unwrap(),
        ["alpha", "beta", "gamma", "delta"]
            .iter()
            .map(|s| Cell::Str(s.to_string()))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        table.column("COORD").unwrap(),
        vec![
            Cell::Floats(vec![1.0, 2.0]),
            Cell::Floats(vec![3.0, 4.0]),
            Cell::Floats(vec![5.0, 6.0]),
            Cell::Floats(vec![7.0, 8.0]),
        ]
    );
    // Unsigned-via-TZERO column comes back as physical integers.
    assert_eq!(
        table.column("CNT").unwrap(),
        vec![
            Cell::Int(0),
            Cell::Int(30000),
            Cell::Int(60000),
            Cell::Int(65535)
        ]
    );
    // Variable-length column (1PJ) with an empty row.
    assert_eq!(
        table.column("SAMPLES").unwrap(),
        vec![
            Cell::Ints(vec![10, 20]),
            Cell::Ints(vec![30]),
            Cell::Ints(vec![]),
            Cell::Ints(vec![40, 50, 60]),
        ]
    );
}

#[test]
fn bintable_row_access_matches_column_access() {
    let reader = FitsReader::open(fixtures_dir().join("bintable.fits")).unwrap();
    let table = reader.bintable(1).unwrap();

    let cols: Vec<Vec<Cell>> = (0..table.columns().len())
        .map(|c| table.column_at(c).unwrap())
        .collect();

    for r in 0..table.nrows() {
        let row = table.row(r).unwrap();
        for (c, (cell, column)) in row.iter().zip(&cols).enumerate() {
            assert_eq!(*cell, column[r], "row {r} col {c}");
        }
    }
}

#[test]
fn bintable_column_read_touches_only_that_column() {
    let path = fixtures_dir().join("bintable.fits");
    let reader =
        FitsReader::from_source(CountingSource::new(FileSource::open(&path).unwrap())).unwrap();
    let table = reader.bintable(1).unwrap();

    let before_bytes = reader.source().bytes.load(Ordering::Relaxed);
    let before_calls = reader.source().calls.load(Ordering::Relaxed);

    // FLUX is a scalar `E` column: 4 bytes per row, 4 rows.
    let _ = table.column("FLUX").unwrap();

    let bytes = reader.source().bytes.load(Ordering::Relaxed) - before_bytes;
    let calls = reader.source().calls.load(Ordering::Relaxed) - before_calls;
    assert_eq!(calls, 4, "one read per row");
    assert_eq!(bytes, 4 * 4, "only the 4-byte FLUX field per row");
}

#[test]
fn bintable_variable_length_column_reads_descriptor_field_plus_heap_only() {
    let path = fixtures_dir().join("bintable.fits");
    let reader =
        FitsReader::from_source(CountingSource::new(FileSource::open(&path).unwrap())).unwrap();
    let table = reader.bintable(1).unwrap();

    let b0 = reader.source().bytes.load(Ordering::Relaxed);
    let c0 = reader.source().calls.load(Ordering::Relaxed);
    let _ = table.column("SAMPLES").unwrap();
    let bytes = reader.source().bytes.load(Ordering::Relaxed) - b0;
    let calls = reader.source().calls.load(Ordering::Relaxed) - c0;

    // 4 descriptor reads (8 bytes each) + one heap read per non-empty row
    // (rows 0,1,3 -> 8 + 4 + 12 = 24 heap bytes).
    assert_eq!(calls, 4 + 3);
    assert_eq!(bytes, 4 * 8 + 24);
}

#[test]
fn ascii_table_columns_decode_including_null() {
    let reader = FitsReader::open(fixtures_dir().join("ascii_table.fits")).unwrap();
    let table = reader.ascii_table(1).unwrap();
    assert_eq!(table.nrows(), 3);

    assert_eq!(
        table.column("SEQ").unwrap(),
        vec![Cell::Int(1), Cell::Int(2), Cell::Int(3)]
    );
    assert_eq!(
        table.column("MAG").unwrap(),
        vec![Cell::Float(1.234), Cell::Null, Cell::Float(12.5)]
    );
    assert_eq!(
        table.column("LABEL").unwrap(),
        vec![
            Cell::Str("hydrogen".to_string()),
            Cell::Str("helium".to_string()),
            Cell::Str("lithium".to_string()),
        ]
    );
}

#[test]
fn ascii_table_column_read_touches_only_that_column() {
    let path = fixtures_dir().join("ascii_table.fits");
    let reader =
        FitsReader::from_source(CountingSource::new(FileSource::open(&path).unwrap())).unwrap();
    let table = reader.ascii_table(1).unwrap();

    let b0 = reader.source().bytes.load(Ordering::Relaxed);
    let c0 = reader.source().calls.load(Ordering::Relaxed);
    let _ = table.column("MAG").unwrap(); // F8.3 -> 8 bytes/row
    let bytes = reader.source().bytes.load(Ordering::Relaxed) - b0;
    let calls = reader.source().calls.load(Ordering::Relaxed) - c0;
    assert_eq!(calls, 3);
    assert_eq!(bytes, 3 * 8);
}

#[test]
fn wrong_table_kind_is_rejected() {
    let reader = FitsReader::open(fixtures_dir().join("bintable.fits")).unwrap();
    assert!(reader.ascii_table(1).is_err());
    assert!(reader.bintable(0).is_err()); // primary is an image
}
