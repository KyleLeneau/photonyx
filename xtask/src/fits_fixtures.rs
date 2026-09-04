//! Generates the deterministic synthetic FITS corpus described in ADR 006 (O5).
//!
//! This is a small, hand-rolled FITS writer independent of `px-fits` itself — the
//! whole point is to have fixtures that exist *before* `px-fits` can write anything
//! (write support is ADR 006 Phase 5). Every card is a plain-ASCII 80-byte record and
//! every data unit is padded to a multiple of 2880 bytes per the FITS Standard 4.0,
//! section 3.3.2 (block structure) and section 4.1 (card images).
//!
//! Regenerating must be byte-identical: all "random" data is seeded with a fixed
//! SplitMix64 stream, never `std::time` or OS randomness.

use anyhow::Result;
use xshell::Shell;

use crate::{flags, project_root};

const BLOCK: usize = 2880;

impl flags::FitsFixtures {
    pub(crate) fn run(&self, sh: &Shell) -> Result<()> {
        let root = project_root();
        let fixtures_dir = root.join("crates/px-fits/tests/fixtures");
        let invalid_dir = fixtures_dir.join("invalid");
        sh.create_dir(&fixtures_dir)?;
        sh.create_dir(&invalid_dir)?;

        let mut written = Vec::new();
        for (name, bytes) in valid_fixtures() {
            let path = fixtures_dir.join(name);
            sh.write_file(&path, &bytes)?;
            written.push(path);
        }
        for (name, bytes) in invalid_fixtures() {
            let path = invalid_dir.join(name);
            sh.write_file(&path, &bytes)?;
            written.push(path);
        }

        println!(
            "wrote {} fixture files to {:?}",
            written.len(),
            fixtures_dir
        );
        for path in &written {
            println!("  {:?}", path.strip_prefix(&root).unwrap_or(path));
        }

        Ok(())
    }
}

// ---------------------------------------------------------------------------
// Deterministic pseudo-random source (SplitMix64) — no external `rand` dependency
// needed for fixture generation, and its output is stable across platforms/versions.
// ---------------------------------------------------------------------------

struct SplitMix64(u64);

impl SplitMix64 {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next_u64(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E3779B97F4A7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58476D1CE4E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D049BB133111EB);
        z ^ (z >> 31)
    }
}

// ---------------------------------------------------------------------------
// Card / header building
// ---------------------------------------------------------------------------

/// Accumulates 80-byte card images and pads the finished header to a 2880-byte
/// boundary with trailing space-filled cards (FITS Standard 4.0 §4.4.1).
struct HeaderBuf(Vec<u8>);

impl HeaderBuf {
    fn new() -> Self {
        Self(Vec::new())
    }

    fn push_line(&mut self, line: &str) {
        let mut bytes = line.as_bytes().to_vec();
        assert!(bytes.len() <= 80, "card exceeds 80 bytes: {line:?}");
        bytes.resize(80, b' ');
        self.0.extend_from_slice(&bytes);
    }

    fn logical(&mut self, key: &str, val: bool, comment: &str) {
        self.push_line(&format!(
            "{:<8}= {:>20} / {}",
            key,
            if val { "T" } else { "F" },
            comment
        ));
    }

    fn integer(&mut self, key: &str, val: i64, comment: &str) {
        self.push_line(&format!("{key:<8}= {val:>20} / {comment}"));
    }

    fn float(&mut self, key: &str, val: f64, comment: &str) {
        // Fixed-format float: exponential notation, right-justified in the 20-char
        // value field (columns 11-30).
        self.push_line(&format!("{key:<8}= {val:>20.6E} / {comment}"));
    }

    fn string(&mut self, key: &str, val: &str, comment: &str) {
        let escaped = val.replace('\'', "''");
        let quoted = format!("'{escaped:<8}'");
        self.push_line(&format!("{key:<8}= {quoted:<18} / {comment}"));
    }

    /// A long string spread across `CONTINUE` cards per the OGIP long-string
    /// convention: the initial card's value ends with `&`, and each `CONTINUE`
    /// card supplies the next chunk, also `&`-terminated except the last. The
    /// leading comment is carried only on the first card — there is no room
    /// left in an 80-byte card for both a full-width chunk and a comment.
    fn long_string(&mut self, key: &str, val: &str, comment: &str) {
        // Prefix ("KEY     = " / "CONTINUE  ") is 10 chars; quotes + possible
        // trailing '&' take 3; that leaves headroom under the 80-byte limit.
        const CHUNK: usize = 60;
        let chars: Vec<char> = val.chars().collect();
        let mut chunks: Vec<String> = chars
            .chunks(CHUNK)
            .map(|c| c.iter().collect::<String>())
            .collect();
        if chunks.is_empty() {
            chunks.push(String::new());
        }
        let last = chunks.len() - 1;
        for (i, chunk) in chunks.iter().enumerate() {
            let terminated = if i == last {
                chunk.clone()
            } else {
                format!("{chunk}&")
            };
            let escaped = terminated.replace('\'', "''");
            let quoted = format!("'{escaped}'");
            if i == 0 {
                self.push_line(&format!("{key:<8}= {quoted}"));
            } else {
                self.push_line(&format!("CONTINUE  {quoted}"));
            }
        }
        // The comment card must come after the full CONTINUE chain: a
        // CONTINUE card must immediately follow the card it continues (the
        // OGIP long-string convention), so nothing may be interleaved.
        self.comment(comment);
    }

    fn comment(&mut self, text: &str) {
        self.push_line(&format!("COMMENT {text}"));
    }

    fn end(mut self) -> Vec<u8> {
        self.push_line("END");
        let rem = self.0.len() % BLOCK;
        if rem != 0 {
            self.0.resize(self.0.len() + (BLOCK - rem), b' ');
        }
        self.0
    }
}

fn pad_data(mut data: Vec<u8>) -> Vec<u8> {
    let rem = data.len() % BLOCK;
    if rem != 0 {
        data.resize(data.len() + (BLOCK - rem), 0u8);
    }
    data
}

/// One logical HDU: header cards already terminated/padded, plus padded data bytes
/// (empty for a headerless-data primary, e.g. the `NAXIS = 0` case).
struct RawHdu {
    header: Vec<u8>,
    data: Vec<u8>,
}

fn assemble(hdus: Vec<RawHdu>) -> Vec<u8> {
    let mut out = Vec::new();
    for hdu in hdus {
        out.extend_from_slice(&hdu.header);
        out.extend_from_slice(&hdu.data);
    }
    out
}

// ---------------------------------------------------------------------------
// Image HDU builders, one per BITPIX
// ---------------------------------------------------------------------------

fn primary_header(bitpix: i64, naxis: &[i64], extra: impl FnOnce(&mut HeaderBuf)) -> HeaderBuf {
    let mut h = HeaderBuf::new();
    h.logical("SIMPLE", true, "conforms to FITS standard");
    h.integer("BITPIX", bitpix, "bits per data value");
    h.integer("NAXIS", naxis.len() as i64, "number of axes");
    for (i, n) in naxis.iter().enumerate() {
        h.integer(&format!("NAXIS{}", i + 1), *n, "axis length");
    }
    extra(&mut h);
    h
}

fn extension_header(bitpix: i64, naxis: &[i64], extra: impl FnOnce(&mut HeaderBuf)) -> HeaderBuf {
    let mut h = HeaderBuf::new();
    h.string("XTENSION", "IMAGE", "image extension");
    h.integer("BITPIX", bitpix, "bits per data value");
    h.integer("NAXIS", naxis.len() as i64, "number of axes");
    for (i, n) in naxis.iter().enumerate() {
        h.integer(&format!("NAXIS{}", i + 1), *n, "axis length");
    }
    h.integer("PCOUNT", 0, "no group parameters");
    h.integer("GCOUNT", 1, "one data group");
    extra(&mut h);
    h
}

fn npix(naxis: &[i64]) -> usize {
    naxis.iter().product::<i64>().max(0) as usize
}

fn gen_u8(rng: &mut SplitMix64, n: usize) -> Vec<u8> {
    (0..n).map(|_| (rng.next_u64() & 0xFF) as u8).collect()
}

fn gen_i16_be(rng: &mut SplitMix64, n: usize, range: std::ops::RangeInclusive<i16>) -> Vec<u8> {
    let span = (*range.end() as i64 - *range.start() as i64 + 1) as u64;
    let mut out = Vec::with_capacity(n * 2);
    for _ in 0..n {
        let v = *range.start() as i64 + (rng.next_u64() % span) as i64;
        out.extend_from_slice(&(v as i16).to_be_bytes());
    }
    out
}

fn gen_u16_as_i16_bzero(rng: &mut SplitMix64, n: usize) -> Vec<u8> {
    // Physical value = raw_i16 + 32768 (BZERO). To land in [0, 65535] physically,
    // the stored raw value spans the full i16 range.
    let mut out = Vec::with_capacity(n * 2);
    for _ in 0..n {
        let raw = (rng.next_u64() & 0xFFFF) as i16;
        out.extend_from_slice(&raw.to_be_bytes());
    }
    out
}

fn gen_i32_be(rng: &mut SplitMix64, n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n * 4);
    for _ in 0..n {
        let v = (rng.next_u64() & 0xFFFF_FFFF) as i32;
        out.extend_from_slice(&v.to_be_bytes());
    }
    out
}

fn gen_i64_be(rng: &mut SplitMix64, n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n * 8);
    for _ in 0..n {
        out.extend_from_slice(&(rng.next_u64() as i64).to_be_bytes());
    }
    out
}

fn gen_f32_be(rng: &mut SplitMix64, n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n * 4);
    for _ in 0..n {
        let v = (rng.next_u64() as f32 / u64::MAX as f32) * 1000.0;
        out.extend_from_slice(&v.to_be_bytes());
    }
    out
}

fn gen_f64_be(rng: &mut SplitMix64, n: usize) -> Vec<u8> {
    let mut out = Vec::with_capacity(n * 8);
    for _ in 0..n {
        let v = (rng.next_u64() as f64 / u64::MAX as f64) * 1000.0;
        out.extend_from_slice(&v.to_be_bytes());
    }
    out
}

// ---------------------------------------------------------------------------
// The corpus
// ---------------------------------------------------------------------------

fn valid_fixtures() -> Vec<(&'static str, Vec<u8>)> {
    let mut out = Vec::new();
    let mut rng = SplitMix64::new(0x0050_5846_4954_5300); // "PXFITS\0" as a seed

    // BITPIX = 8, 2D
    {
        let naxis = [20i64, 16];
        let header = primary_header(8, &naxis, |h| {
            h.string("OBJECT", "M42", "target");
            h.comment("synthetic fixture: bitpix8_2d");
        })
        .end();
        let data = pad_data(gen_u8(&mut rng, npix(&naxis)));
        out.push((
            "bitpix8_2d_20x16.fits",
            assemble(vec![RawHdu { header, data }]),
        ));
    }

    // BITPIX = 16 signed, 2D
    {
        let naxis = [64i64, 48];
        let header = primary_header(16, &naxis, |h| {
            h.comment("synthetic fixture: bitpix16_2d signed");
        })
        .end();
        let data = pad_data(gen_i16_be(&mut rng, npix(&naxis), -1000..=1000));
        out.push((
            "bitpix16_2d_64x48.fits",
            assemble(vec![RawHdu { header, data }]),
        ));
    }

    // BITPIX = 16 with BZERO/BSCALE unsigned convention (D6 in ADR 006)
    {
        let naxis = [32i64, 32];
        let header = primary_header(16, &naxis, |h| {
            h.float("BSCALE", 1.0, "physical = BZERO + BSCALE * raw");
            h.float("BZERO", 32768.0, "unsigned 16-bit convention");
            h.comment("synthetic fixture: unsigned-via-bzero");
        })
        .end();
        let data = pad_data(gen_u16_as_i16_bzero(&mut rng, npix(&naxis)));
        out.push((
            "bitpix16_unsigned_bzero_2d_32x32.fits",
            assemble(vec![RawHdu { header, data }]),
        ));
    }

    // BITPIX = 32, 2D
    {
        let naxis = [16i64, 16];
        let header = primary_header(32, &naxis, |h| {
            h.comment("synthetic fixture: bitpix32_2d");
        })
        .end();
        let data = pad_data(gen_i32_be(&mut rng, npix(&naxis)));
        out.push((
            "bitpix32_2d_16x16.fits",
            assemble(vec![RawHdu { header, data }]),
        ));
    }

    // BITPIX = 64, 1D
    {
        let naxis = [100i64];
        let header = primary_header(64, &naxis, |h| {
            h.comment("synthetic fixture: bitpix64_1d");
        })
        .end();
        let data = pad_data(gen_i64_be(&mut rng, npix(&naxis)));
        out.push((
            "bitpix64_1d_100.fits",
            assemble(vec![RawHdu { header, data }]),
        ));
    }

    // BITPIX = -32, 3D cube
    {
        let naxis = [8i64, 8, 4];
        let header = primary_header(-32, &naxis, |h| {
            h.comment("synthetic fixture: bitpix-32_3d cube");
        })
        .end();
        let data = pad_data(gen_f32_be(&mut rng, npix(&naxis)));
        out.push((
            "bitpixneg32_3d_8x8x4.fits",
            assemble(vec![RawHdu { header, data }]),
        ));
    }

    // BITPIX = -64, 2D
    {
        let naxis = [10i64, 10];
        let header = primary_header(-64, &naxis, |h| {
            h.comment("synthetic fixture: bitpix-64_2d");
        })
        .end();
        let data = pad_data(gen_f64_be(&mut rng, npix(&naxis)));
        out.push((
            "bitpixneg64_2d_10x10.fits",
            assemble(vec![RawHdu { header, data }]),
        ));
    }

    // BLANK keyword: BITPIX=16, some raw values equal BLANK
    {
        let naxis = [10i64, 10];
        let header = primary_header(16, &naxis, |h| {
            h.integer("BLANK", -32768, "undefined pixel value");
            h.comment("synthetic fixture: blank pixels present");
        })
        .end();
        let mut raw = gen_i16_be(&mut rng, npix(&naxis), -100..=100);
        // Force a few pixels to BLANK.
        for i in [0usize, 5, 17].iter() {
            let off = i * 2;
            raw[off..off + 2].copy_from_slice(&(-32768i16).to_be_bytes());
        }
        let data = pad_data(raw);
        out.push((
            "blank_bitpix16_2d_10x10.fits",
            assemble(vec![RawHdu { header, data }]),
        ));
    }

    // Multi-extension: primary with NAXIS=0 (no data) + one IMAGE extension
    {
        let primary_header = primary_header(8, &[], |h| {
            h.logical("EXTEND", true, "file may contain extensions");
            h.comment("synthetic fixture: multi-extension primary");
        })
        .end();
        let ext_naxis = [12i64, 9];
        let ext_header = extension_header(16, &ext_naxis, |h| {
            h.comment("synthetic fixture: extension 1");
        })
        .end();
        let ext_data = pad_data(gen_i16_be(&mut rng, npix(&ext_naxis), -500..=500));
        out.push((
            "multi_extension.fits",
            assemble(vec![
                RawHdu {
                    header: primary_header,
                    data: Vec::new(),
                },
                RawHdu {
                    header: ext_header,
                    data: ext_data,
                },
            ]),
        ));
    }

    // Long string via CONTINUE convention
    {
        let naxis = [4i64, 4];
        let long = "This is a deliberately long comment-like value that must be split across \
                     multiple CONTINUE cards per the OGIP convention to exercise the parser.";
        let header = primary_header(8, &naxis, |h| {
            h.long_string("LONGSTR", long, "spans CONTINUE cards");
        })
        .end();
        let data = pad_data(gen_u8(&mut rng, npix(&naxis)));
        out.push((
            "long_string_continue.fits",
            assemble(vec![RawHdu { header, data }]),
        ));
    }

    // Header-heavy, data-light: for header-only-scan laziness assertions. Many
    // COMMENT cards inflate the header to several blocks while the data unit
    // stays small, so a lazy reader's byte count is dominated by header blocks
    // and is easy to assert exactly.
    {
        let naxis = [4i64, 4];
        let mut h = primary_header(16, &naxis, |_h| {});
        for i in 0..80 {
            h.comment(&format!("padding comment line {i} to inflate header size"));
        }
        let header = h.end();
        let data = pad_data(gen_i16_be(&mut rng, npix(&naxis), -10..=10));
        out.push((
            "header_heavy_4x4.fits",
            assemble(vec![RawHdu { header, data }]),
        ));
    }

    out.push(("bintable.fits", bintable_fixture()));
    out.push(("ascii_table.fits", ascii_table_fixture()));

    out
}

/// An empty (`NAXIS = 0`) primary HDU, for files whose payload is an
/// extension.
fn empty_primary() -> RawHdu {
    RawHdu {
        header: primary_header(8, &[], |h| {
            h.logical("EXTEND", true, "file contains extensions");
        })
        .end(),
        data: Vec::new(),
    }
}

/// A `BINTABLE` exercising scalar/vector/string/scaled columns and a
/// variable-length (`1PJ`) column with a real `PCOUNT` heap.
fn bintable_fixture() -> Vec<u8> {
    // Column layout (bytes per row): J(4) E(4) 6A(6) 2D(16) I(2) 1PJ(8) = 40.
    const ROW: i64 = 40;
    const NROWS: i64 = 4;

    let ids: [i32; 4] = [1, 2, 3, 4];
    let flux: [f32; 4] = [1.5, 2.5, 3.5, 4.5];
    let names: [&str; 4] = ["alpha", "beta", "gamma", "delta"];
    let coord: [[f64; 2]; 4] = [[1.0, 2.0], [3.0, 4.0], [5.0, 6.0], [7.0, 8.0]];
    // Physical [0, 30000, 60000, 65535] via TZERO = 32768 on a signed I column.
    let cnt_phys: [i64; 4] = [0, 30000, 60000, 65535];
    // Variable-length rows.
    let vla: [&[i32]; 4] = [&[10, 20], &[30], &[], &[40, 50, 60]];

    let mut rows = Vec::new();
    let mut heap = Vec::new();
    for r in 0..4usize {
        rows.extend_from_slice(&ids[r].to_be_bytes());
        rows.extend_from_slice(&flux[r].to_be_bytes());
        let mut name = [b' '; 6];
        let bytes = names[r].as_bytes();
        name[..bytes.len()].copy_from_slice(bytes);
        rows.extend_from_slice(&name);
        rows.extend_from_slice(&coord[r][0].to_be_bytes());
        rows.extend_from_slice(&coord[r][1].to_be_bytes());
        rows.extend_from_slice(&((cnt_phys[r] - 32768) as i16).to_be_bytes());
        // 1PJ descriptor: [nelem, byte offset into heap].
        let nelem = vla[r].len() as i32;
        let offset = heap.len() as i32;
        rows.extend_from_slice(&nelem.to_be_bytes());
        rows.extend_from_slice(&offset.to_be_bytes());
        for &v in vla[r] {
            heap.extend_from_slice(&v.to_be_bytes());
        }
    }
    let pcount = heap.len() as i64;

    let mut h = HeaderBuf::new();
    h.string("XTENSION", "BINTABLE", "binary table extension");
    h.integer("BITPIX", 8, "bits per data value");
    h.integer("NAXIS", 2, "2-dimensional table");
    h.integer("NAXIS1", ROW, "width of table row in bytes");
    h.integer("NAXIS2", NROWS, "number of rows");
    h.integer("PCOUNT", pcount, "size of heap in bytes");
    h.integer("GCOUNT", 1, "one data group");
    h.integer("TFIELDS", 6, "number of columns");
    h.string("TTYPE1", "ID", "row id");
    h.string("TFORM1", "J", "32-bit integer");
    h.string("TTYPE2", "FLUX", "measured flux");
    h.string("TFORM2", "E", "single precision float");
    h.string("TUNIT2", "Jy", "janskys");
    h.string("TTYPE3", "NAME", "object name");
    h.string("TFORM3", "6A", "6-char string");
    h.string("TTYPE4", "COORD", "x/y position");
    h.string("TFORM4", "2D", "two doubles");
    h.string("TTYPE5", "CNT", "unsigned count via TZERO");
    h.string("TFORM5", "I", "16-bit integer");
    h.integer("TSCAL5", 1, "no scaling");
    h.integer("TZERO5", 32768, "unsigned 16-bit offset");
    h.string("TTYPE6", "SAMPLES", "variable-length samples");
    h.string("TFORM6", "1PJ(3)", "var-length 32-bit ints, max 3");
    h.comment("synthetic fixture: bintable with a variable-length column");
    let header = h.end();

    let mut data = rows;
    data.extend_from_slice(&heap);
    assemble(vec![
        empty_primary(),
        RawHdu {
            header,
            data: pad_data(data),
        },
    ])
}

/// An ASCII `TABLE` with an integer, a float, and a string column, plus one
/// all-blank (null) cell.
fn ascii_table_fixture() -> Vec<u8> {
    // Row layout: SEQ I5 @1, MAG F8.3 @7, LABEL A10 @16 -> row width 25.
    const ROW: usize = 25;
    let rows: [(&str, &str, &str); 3] = [
        ("    1", "  1.234", "hydrogen  "),
        ("    2", "       ", "helium    "), // blank MAG -> null
        ("    3", " 12.500", "lithium   "),
    ];

    let mut data = Vec::new();
    for (seq, mag, label) in rows {
        let mut line = vec![b' '; ROW];
        line[0..5].copy_from_slice(seq.as_bytes());
        line[6..13].copy_from_slice(mag.as_bytes());
        line[15..25].copy_from_slice(label.as_bytes());
        data.extend_from_slice(&line);
    }

    let mut h = HeaderBuf::new();
    h.string("XTENSION", "TABLE", "ASCII table extension");
    h.integer("BITPIX", 8, "bits per data value");
    h.integer("NAXIS", 2, "2-dimensional table");
    h.integer("NAXIS1", ROW as i64, "width of table row in bytes");
    h.integer("NAXIS2", 3, "number of rows");
    h.integer("PCOUNT", 0, "no heap");
    h.integer("GCOUNT", 1, "one data group");
    h.integer("TFIELDS", 3, "number of columns");
    h.string("TTYPE1", "SEQ", "sequence number");
    h.integer("TBCOL1", 1, "start column");
    h.string("TFORM1", "I5", "integer, 5 chars");
    h.string("TTYPE2", "MAG", "magnitude");
    h.integer("TBCOL2", 7, "start column");
    h.string("TFORM2", "F8.3", "fixed float");
    h.string("TTYPE3", "LABEL", "element name");
    h.integer("TBCOL3", 16, "start column");
    h.string("TFORM3", "A10", "10-char string");
    h.comment("synthetic fixture: ASCII table with a null MAG cell");
    let header = h.end();

    assemble(vec![
        empty_primary(),
        RawHdu {
            header,
            data: pad_data(data),
        },
    ])
}

fn invalid_fixtures() -> Vec<(&'static str, Vec<u8>)> {
    let mut out = Vec::new();
    let mut rng = SplitMix64::new(0x00BA_DF17_5EED);

    // Truncated data unit: header declares more data than is actually present.
    {
        let naxis = [100i64, 100];
        let header = primary_header(16, &naxis, |h| {
            h.comment("invalid fixture: truncated data unit");
        })
        .end();
        let full = gen_i16_be(&mut rng, npix(&naxis), -10..=10);
        // Keep only the first quarter of the (unpadded) data.
        let truncated = full[..full.len() / 4].to_vec();
        out.push((
            "truncated_data.fits",
            assemble(vec![RawHdu {
                header,
                data: truncated,
            }]),
        ));
    }

    // Missing END: a full header block of otherwise-valid cards with no END card
    // before EOF.
    {
        let mut h = HeaderBuf::new();
        h.logical("SIMPLE", true, "conforms to FITS standard");
        h.integer("BITPIX", 8, "bits per data value");
        h.integer("NAXIS", 0, "number of axes");
        // Pad out a full block with COMMENT cards but deliberately omit `.end()`
        // (which would append END + pad). We pad manually without END.
        while h.0.len() < BLOCK {
            h.comment("padding without a terminating END card");
        }
        h.0.truncate(BLOCK);
        out.push(("missing_end.fits", h.0));
    }

    // Malformed card: byte 9 is not '=' for a keyword that requires it, and the
    // bytes are not valid in any recognized card syntax.
    {
        let mut h = HeaderBuf::new();
        h.logical("SIMPLE", true, "conforms to FITS standard");
        h.integer("BITPIX", 8, "bits per data value");
        h.integer("NAXIS", 0, "number of axes");
        h.push_line("BADCARD!!!!not a valid value syntax at all,,,,");
        let header = h.end();
        out.push(("bad_card_syntax.fits", header));
    }

    // NAXIS overflow: declares an implausibly large axis length. A conforming
    // reader must reject this before attempting to allocate, per the allocation
    // guard in ADR 006 O6.
    {
        let mut h = HeaderBuf::new();
        h.logical("SIMPLE", true, "conforms to FITS standard");
        h.integer("BITPIX", 8, "bits per data value");
        h.integer("NAXIS", 2, "number of axes");
        h.integer("NAXIS1", 999_999_999_999, "implausible axis length");
        h.integer("NAXIS2", 999_999_999_999, "implausible axis length");
        let header = h.end();
        out.push(("naxis_overflow.fits", header));
    }

    // Negative NAXISn: invalid per FITS Standard 4.0 §4.4.1 (NAXISn >= 0).
    {
        let mut h = HeaderBuf::new();
        h.logical("SIMPLE", true, "conforms to FITS standard");
        h.integer("BITPIX", 8, "bits per data value");
        h.integer("NAXIS", 1, "number of axes");
        h.integer("NAXIS1", -10, "invalid: negative axis length");
        let header = h.end();
        out.push(("negative_naxis.fits", header));
    }

    // Invalid BITPIX: 3 is not one of the standard's permitted values.
    {
        let mut h = HeaderBuf::new();
        h.logical("SIMPLE", true, "conforms to FITS standard");
        h.integer("BITPIX", 3, "invalid: not a permitted BITPIX value");
        h.integer("NAXIS", 0, "number of axes");
        let header = h.end();
        out.push(("invalid_bitpix.fits", header));
    }

    out
}
