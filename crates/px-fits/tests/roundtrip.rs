//! ADR 006 Phase 5 gate: write -> read -> compare, bit-exact, for every
//! `BITPIX`, dimensionality, and scaling combination; plus `update_header`'s
//! in-place and full-rewrite paths.

use std::path::PathBuf;

use px_fits::card::Value;
use px_fits::header::BitPix;
use px_fits::reader::FitsReader;
use px_fits::{CardEdit, FitsWriter, HeaderBuilder, Region, update_header};

fn tmp(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join("px-fits-roundtrip");
    std::fs::create_dir_all(&dir).unwrap();
    dir.join(name)
}

/// Deterministic pseudo-random bytes, reused so tests don't depend on `rand`.
struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

fn write_image<T: px_fits::Pixel>(path: &std::path::Path, h: &HeaderBuilder, data: &[T]) {
    let mut w = FitsWriter::create(path).unwrap();
    w.write_image(h, data).unwrap();
    w.finish().unwrap();
}

#[test]
fn roundtrip_every_bitpix_and_dimensionality() {
    let mut rng = Rng(0x5052_5F52_4F55_4E44);
    let shapes: &[&[u64]] = &[&[17], &[8, 5], &[4, 3, 2]];

    for shape in shapes {
        let n: usize = shape.iter().product::<u64>() as usize;

        // --- integer BITPIX ---
        macro_rules! int_case {
            ($t:ty, $bp:expr) => {{
                let data: Vec<$t> = (0..n).map(|_| rng.next() as $t).collect();
                let h = HeaderBuilder::primary_image($bp, shape).unwrap();
                let path = tmp(&format!("int_{}_{:?}.fits", stringify!($t), shape));
                write_image(&path, &h, &data);

                let reader = FitsReader::open(&path).unwrap();
                let img = reader.primary_image().unwrap();
                assert_eq!(img.bitpix(), $bp);
                assert_eq!(
                    img.shape(),
                    shape
                        .iter()
                        .map(|&x| x as usize)
                        .collect::<Vec<_>>()
                        .as_slice()
                );
                assert_eq!(
                    img.read_full::<$t>().unwrap(),
                    data,
                    "{} {:?}",
                    stringify!($t),
                    shape
                );
            }};
        }
        int_case!(u8, BitPix::U8);
        int_case!(i16, BitPix::I16);
        int_case!(i32, BitPix::I32);
        int_case!(i64, BitPix::I64);

        // --- float BITPIX (bit-exact via to_bits) ---
        macro_rules! float_case {
            ($t:ty, $bp:expr, $bits:ty) => {{
                let data: Vec<$t> = (0..n).map(|i| (rng.next() as $t) / 7.0 - i as $t).collect();
                let h = HeaderBuilder::primary_image($bp, shape).unwrap();
                let path = tmp(&format!("flt_{}_{:?}.fits", stringify!($t), shape));
                write_image(&path, &h, &data);

                let back = FitsReader::open(&path)
                    .unwrap()
                    .primary_image()
                    .unwrap()
                    .read_full::<$t>()
                    .unwrap();
                let a: Vec<$bits> = data.iter().map(|v| v.to_bits()).collect();
                let b: Vec<$bits> = back.iter().map(|v| v.to_bits()).collect();
                assert_eq!(a, b, "{} {:?}", stringify!($t), shape);
            }};
        }
        float_case!(f32, BitPix::F32, u32);
        float_case!(f64, BitPix::F64, u64);
    }
}

#[test]
fn roundtrip_unsigned16_via_bzero_card() {
    // Write i16 storage values, declare BZERO=32768, read back as u16.
    let want: Vec<u16> = vec![0, 1, 32767, 32768, 40000, 65535];
    let stored: Vec<i16> = want.iter().map(|&u| (u as i32 - 32768) as i16).collect();

    let h = HeaderBuilder::primary_image(BitPix::I16, &[6])
        .unwrap()
        .set_f64("BZERO", 32768.0, Some("unsigned 16-bit"))
        .unwrap()
        .set_f64("BSCALE", 1.0, None)
        .unwrap();
    let path = tmp("u16_bzero.fits");
    write_image(&path, &h, &stored);

    let reader = FitsReader::open(&path).unwrap();
    let img = reader.primary_image().unwrap();
    // BZERO is applied on read, so the physical values come back regardless of
    // the requested integer width (i32 is wide enough to hold them exactly).
    assert_eq!(img.read_full::<u16>().unwrap(), want);
    assert_eq!(
        img.read_full::<i32>().unwrap(),
        want.iter().map(|&u| u as i32).collect::<Vec<_>>()
    );
}

#[test]
fn roundtrip_real_bscale_bzero() {
    // physical = 10 + 0.25 * raw
    let raw: Vec<i32> = vec![-8, 0, 4, 400, -1000];
    let h = HeaderBuilder::primary_image(BitPix::I32, &[5])
        .unwrap()
        .set_f64("BSCALE", 0.25, None)
        .unwrap()
        .set_f64("BZERO", 10.0, None)
        .unwrap();
    let path = tmp("scaled.fits");
    write_image(&path, &h, &raw);

    let got = FitsReader::open(&path)
        .unwrap()
        .primary_image()
        .unwrap()
        .read_full::<f64>()
        .unwrap();
    let want: Vec<f64> = raw.iter().map(|&r| 10.0 + 0.25 * r as f64).collect();
    assert_eq!(got, want);
}

#[test]
fn roundtrip_blank_sentinel() {
    let stored: Vec<i16> = vec![1, -32768, 3, -32768, 5];
    let h = HeaderBuilder::primary_image(BitPix::I16, &[5])
        .unwrap()
        .set_i64("BLANK", -32768, Some("undefined"))
        .unwrap();
    let path = tmp("blank.fits");
    write_image(&path, &h, &stored);

    let img_reader = FitsReader::open(&path).unwrap();
    let img = img_reader.primary_image().unwrap();
    let as_f32 = img.read_full::<f32>().unwrap();
    assert!(as_f32[1].is_nan() && as_f32[3].is_nan());
    assert_eq!(as_f32[0], 1.0);
    assert_eq!(img.read_full::<i16>().unwrap(), stored); // integer target: passthrough
}

#[test]
fn roundtrip_multi_hdu_primary_plus_image_extension() {
    let path = tmp("multi.fits");
    let mut w = FitsWriter::create(&path).unwrap();

    let primary = HeaderBuilder::primary_image(BitPix::U8, &[]).unwrap();
    w.write_image::<u8>(&primary, &[]).unwrap();

    let ext = HeaderBuilder::image_ext(BitPix::I16, &[3, 2])
        .unwrap()
        .set_str("EXTNAME", "SCI", None)
        .unwrap();
    let ext_data: Vec<i16> = vec![10, 20, 30, 40, 50, 60];
    w.write_image(&ext, &ext_data).unwrap();
    w.finish().unwrap();

    let reader = FitsReader::open(&path).unwrap();
    assert_eq!(reader.hdu_count().unwrap(), 2);
    assert_eq!(reader.primary_image().unwrap().len(), 0);
    let ext_img = reader.image(1).unwrap();
    assert_eq!(ext_img.shape(), &[3, 2]);
    assert_eq!(ext_img.read_full::<i16>().unwrap(), ext_data);
    assert_eq!(
        ext_img.header().get_string("EXTNAME").as_deref(),
        Some("SCI")
    );
}

#[test]
fn roundtrip_region_read_on_written_file() {
    let mut rng = Rng(99);
    let data: Vec<i32> = (0..(20 * 12)).map(|_| rng.next() as i32).collect();
    let h = HeaderBuilder::primary_image(BitPix::I32, &[20, 12]).unwrap();
    let path = tmp("region_src.fits");
    write_image(&path, &h, &data);

    let reader = FitsReader::open(&path).unwrap();
    let img = reader.primary_image().unwrap();
    let region = Region::rect(4, 3, 8, 6);
    let got = img.read_region::<i32>(&region).unwrap();

    let mut want = Vec::new();
    for row in 3..9 {
        for col in 4..12 {
            want.push(data[row * 20 + col]);
        }
    }
    assert_eq!(got, want);
}

#[test]
fn update_header_in_place_when_block_count_unchanged() {
    let h = HeaderBuilder::primary_image(BitPix::I16, &[4, 4])
        .unwrap()
        .set_str("FILTER", "L", Some("clear"))
        .unwrap();
    let data: Vec<i16> = (0..16).collect();
    let path = tmp("update_inplace.fits");
    write_image(&path, &h, &data);
    let before_len = std::fs::metadata(&path).unwrap().len();

    update_header(
        &path,
        0,
        &[
            CardEdit::set_with_comment("FILTER", Value::String("Ha".into()), "narrowband"),
            CardEdit::set("EXPTIME", Value::Float(300.0)),
        ],
    )
    .unwrap();

    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        before_len,
        "file grew"
    );
    let reader = FitsReader::open(&path).unwrap();
    let img = reader.primary_image().unwrap();
    assert_eq!(img.header().get_string("FILTER").as_deref(), Some("Ha"));
    assert_eq!(img.header().get_f64("EXPTIME"), Some(300.0));
    // data untouched
    assert_eq!(img.read_full::<i16>().unwrap(), data);
}

#[test]
fn update_header_rewrites_file_when_header_grows_a_block() {
    let h = HeaderBuilder::primary_image(BitPix::U8, &[10, 10]).unwrap();
    let data: Vec<u8> = (0..100).map(|i| i as u8).collect();
    let path = tmp("update_grow.fits");
    write_image(&path, &h, &data);
    let before_len = std::fs::metadata(&path).unwrap().len();

    // Add enough cards to push the header from 1 block (36 cards) to 2.
    let edits: Vec<CardEdit> = (0..40)
        .map(|i| CardEdit::set(format!("KEY{i:03}"), Value::Integer(i)))
        .collect();
    update_header(&path, 0, &edits).unwrap();

    let after_len = std::fs::metadata(&path).unwrap().len();
    assert_eq!(
        after_len,
        before_len + 2880,
        "header should have grown one block"
    );

    let reader = FitsReader::open(&path).unwrap();
    let img = reader.primary_image().unwrap();
    for i in 0..40 {
        assert_eq!(img.header().get_i64(&format!("KEY{i:03}")), Some(i));
    }
    assert_eq!(img.read_full::<u8>().unwrap(), data);

    // No temp file left behind for this target (the temp name derives from
    // the target file name, so other tests sharing the dir don't disturb it).
    let tmp_name = format!(
        ".{}.px-fits-tmp",
        path.file_name().unwrap().to_string_lossy()
    );
    assert!(
        !path.parent().unwrap().join(&tmp_name).exists(),
        "temp file {tmp_name} not cleaned up"
    );
}

#[test]
fn update_header_shrinks_file_when_header_loses_a_block() {
    let mut h = HeaderBuilder::primary_image(BitPix::U8, &[10, 10]).unwrap();
    for i in 0..40 {
        h = h.set_i64(&format!("PAD{i:03}"), i, None).unwrap();
    }
    let data: Vec<u8> = vec![7; 100];
    let path = tmp("update_shrink.fits");
    write_image(&path, &h, &data);
    let before_len = std::fs::metadata(&path).unwrap().len();

    let edits: Vec<CardEdit> = (0..40)
        .map(|i| CardEdit::remove(format!("PAD{i:03}")))
        .collect();
    update_header(&path, 0, &edits).unwrap();

    assert_eq!(
        std::fs::metadata(&path).unwrap().len(),
        before_len - 2880,
        "header should have shrunk one block"
    );
    let reader = FitsReader::open(&path).unwrap();
    let img = reader.primary_image().unwrap();
    assert!(img.header().get("PAD000").is_none());
    assert_eq!(img.read_full::<u8>().unwrap(), data);
}

#[test]
fn update_header_refuses_structural_keywords() {
    let h = HeaderBuilder::primary_image(BitPix::I16, &[4, 4]).unwrap();
    let path = tmp("update_structural.fits");
    write_image(&path, &h, &[0i16; 16]);

    for kw in ["NAXIS1", "BITPIX", "SIMPLE"] {
        assert!(
            update_header(&path, 0, &[CardEdit::set(kw, Value::Integer(1))]).is_err(),
            "editing {kw} should be rejected"
        );
        assert!(update_header(&path, 0, &[CardEdit::remove(kw)]).is_err());
    }
}

#[test]
fn update_header_on_extension_hdu() {
    let path = tmp("update_ext.fits");
    let mut w = FitsWriter::create(&path).unwrap();
    w.write_image::<u8>(&HeaderBuilder::primary_image(BitPix::U8, &[]).unwrap(), &[])
        .unwrap();
    w.write_image::<i16>(
        &HeaderBuilder::image_ext(BitPix::I16, &[2, 2]).unwrap(),
        &[1, 2, 3, 4],
    )
    .unwrap();
    w.finish().unwrap();

    update_header(
        &path,
        1,
        &[CardEdit::set("EXTNAME", Value::String("SCI".into()))],
    )
    .unwrap();

    let reader = FitsReader::open(&path).unwrap();
    assert_eq!(
        reader
            .image(1)
            .unwrap()
            .header()
            .get_string("EXTNAME")
            .as_deref(),
        Some("SCI")
    );
    assert_eq!(
        reader.image(1).unwrap().read_full::<i16>().unwrap(),
        vec![1, 2, 3, 4]
    );
}

// --- Tables (ADR 006 P6-T6) ------------------------------------------------

use px_fits::Cell;
use px_fits::table::{AsciiTableBuilder, BinTableBuilder};

#[test]
fn roundtrip_bintable_all_column_kinds_including_variable_length() {
    let vla: [&[i64]; 3] = [&[10, 20, 30], &[], &[40]];
    let mut t = BinTableBuilder::new()
        .column("ID", "J")
        .unwrap()
        .column("OK", "L")
        .unwrap()
        .column("NAME", "6A")
        .unwrap()
        .column("XY", "2E")
        .unwrap()
        .column("BIG", "K")
        .unwrap()
        .column_full("UCNT", "I", None, Some(1.0), Some(32768.0), None)
        .unwrap()
        .column("VAR", "1PJ(3)")
        .unwrap();
    for r in 0..3i64 {
        t = t
            .push_row(vec![
                Cell::Int(r),
                Cell::Bool(r % 2 == 0),
                Cell::Str(format!("n{r}")),
                Cell::Floats(vec![r as f64 * 0.5, r as f64 * -0.5]),
                Cell::Int(r * 1_000_000_000),
                Cell::Int(20000 + r * 20000),
                Cell::Ints(vla[r as usize].to_vec()),
            ])
            .unwrap();
    }

    let path = tmp("bintable_rt.fits");
    let mut w = FitsWriter::create(&path).unwrap();
    w.write_image::<u8>(&HeaderBuilder::primary_image(BitPix::U8, &[]).unwrap(), &[])
        .unwrap();
    w.write_bintable(&t).unwrap();
    w.finish().unwrap();

    let reader = FitsReader::open(&path).unwrap();
    let table = reader.bintable(1).unwrap();
    assert_eq!(table.nrows(), 3);
    assert_eq!(
        table.column("ID").unwrap(),
        vec![Cell::Int(0), Cell::Int(1), Cell::Int(2)]
    );
    assert_eq!(
        table.column("OK").unwrap(),
        vec![Cell::Bool(true), Cell::Bool(false), Cell::Bool(true)]
    );
    assert_eq!(
        table.column("NAME").unwrap(),
        vec![
            Cell::Str("n0".into()),
            Cell::Str("n1".into()),
            Cell::Str("n2".into())
        ]
    );
    assert_eq!(
        table.column("XY").unwrap(),
        vec![
            Cell::Floats(vec![0.0, 0.0]),
            Cell::Floats(vec![0.5, -0.5]),
            Cell::Floats(vec![1.0, -1.0]),
        ]
    );
    assert_eq!(
        table.column("BIG").unwrap(),
        vec![
            Cell::Int(0),
            Cell::Int(1_000_000_000),
            Cell::Int(2_000_000_000)
        ]
    );
    assert_eq!(
        table.column("UCNT").unwrap(),
        vec![Cell::Int(20000), Cell::Int(40000), Cell::Int(60000)]
    );
    assert_eq!(
        table.column("VAR").unwrap(),
        vec![
            Cell::Ints(vec![10, 20, 30]),
            Cell::Ints(vec![]),
            Cell::Ints(vec![40]),
        ]
    );
}

#[test]
fn roundtrip_ascii_table_with_null() {
    let t = AsciiTableBuilder::new()
        .column("N", "I6")
        .unwrap()
        .column("V", "F10.4")
        .unwrap()
        .column("S", "A8")
        .unwrap()
        .push_row(vec![
            Cell::Int(42),
            Cell::Float(1.2345),
            Cell::Str("pi".into()),
        ])
        .unwrap()
        .push_row(vec![Cell::Int(-7), Cell::Null, Cell::Str("none".into())])
        .unwrap();

    let path = tmp("ascii_rt.fits");
    let mut w = FitsWriter::create(&path).unwrap();
    w.write_image::<u8>(&HeaderBuilder::primary_image(BitPix::U8, &[]).unwrap(), &[])
        .unwrap();
    w.write_ascii_table(&t).unwrap();
    w.finish().unwrap();

    let reader = FitsReader::open(&path).unwrap();
    let table = reader.ascii_table(1).unwrap();
    assert_eq!(
        table.column("N").unwrap(),
        vec![Cell::Int(42), Cell::Int(-7)]
    );
    assert_eq!(
        table.column("V").unwrap(),
        vec![Cell::Float(1.2345), Cell::Null]
    );
    assert_eq!(
        table.column("S").unwrap(),
        vec![Cell::Str("pi".into()), Cell::Str("none".into())]
    );
}

#[test]
fn table_extension_cannot_be_the_first_hdu() {
    let t = BinTableBuilder::new().column("A", "J").unwrap();
    let mut w = FitsWriter::new(std::io::Cursor::new(Vec::new()));
    assert!(w.write_bintable(&t).is_err());
}
