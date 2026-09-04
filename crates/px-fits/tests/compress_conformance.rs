//! ADR 006 Phase 7 gate: our tile decompression against reference files
//! produced by astropy (which wraps the same cfitsio compression code
//! `fpack` uses). Bit-exact for `RICE_1`/`GZIP_1`/`GZIP_2`/`NOCOMPRESS`;
//! region reads decompress only intersecting tiles; unsupported types return
//! a typed error, never a panic.
//!
//! Skips (passing) when neither a system `python3`+astropy nor `uv` is
//! available — same policy as `tests/external_validation.rs`.

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::atomic::{AtomicU64, Ordering};

use px_fits::header::BitPix;
use px_fits::reader::FitsReader;
use px_fits::source::{ByteSource, FileSource};
use px_fits::{CompAlgo, CompressedImageBuilder, FitsError, FitsWriter, HeaderBuilder, Region};

fn scratch(name: &str) -> PathBuf {
    let d = std::env::temp_dir().join("px-fits-compress").join(name);
    let _ = std::fs::remove_dir_all(&d);
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// `python3` if it can import astropy, else `uv run --with astropy`, else
/// `None`.
fn python() -> Option<Vec<String>> {
    let s = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    if Command::new("python3")
        .args(["-c", "import astropy, numpy"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
    {
        return Some(s(&["python3"]));
    }
    if Command::new("uv").arg("--version").output().is_ok() {
        return Some(s(&[
            "uv", "run", "--quiet", "--with", "astropy", "--with", "numpy", "--", "python",
        ]));
    }
    None
}

const GEN: &str = r#"
import sys, numpy as np
from astropy.io import fits

out = sys.argv[1]

def img16():
    a = ((np.arange(60*40) * 37) % 517 - 200).astype(np.int16).reshape(40, 60)
    return a

def img32():
    a = ((np.arange(50*50) * 2654435761) % 1_000_003 - 500_000).astype(np.int32).reshape(50, 50)
    return a

def imgu16():
    a = ((np.arange(48*48) * 991) % 65536).astype(np.uint16).reshape(48, 48)
    return a

def mask():
    a = ((np.arange(32*32) // 7) % 4).astype(np.int16).reshape(32, 32)
    return a

def write(name, data, ctype, tile):
    try:
        h = fits.CompImageHDU(data, compression_type=ctype, tile_shape=tile)
    except TypeError:
        h = fits.CompImageHDU(data, compression_type=ctype, tile_size=list(reversed(tile)))
    fits.HDUList([fits.PrimaryHDU(), h]).writeto(f"{out}/{name}.fits", overwrite=True)

write("rice16", img16(), "RICE_1", (8, 16))
write("rice32", img32(), "RICE_1", (10, 10))
write("riceu16", imgu16(), "RICE_1", (16, 16))
write("gzip1_16", img16(), "GZIP_1", (8, 16))
write("gzip2_16", img16(), "GZIP_2", (8, 16))
write("gzip1_32", img32(), "GZIP_1", (13, 13))
write("nocomp16", img16(), "NOCOMPRESS", (8, 16))
write("plio_mask", mask(), "PLIO_1", (16, 16))
write("hcomp16", img16(), "HCOMPRESS_1", (20, 30))

# Dump the reference arrays as .npy for the Rust side to compare against.
np.save(f"{out}/rice16.npy", img16())
np.save(f"{out}/rice32.npy", img32())
np.save(f"{out}/riceu16.npy", imgu16())
np.save(f"{out}/gzip1_16.npy", img16())
np.save(f"{out}/gzip2_16.npy", img16())
np.save(f"{out}/gzip1_32.npy", img32())
np.save(f"{out}/nocomp16.npy", img16())
print("ok")
"#;

/// Minimal `.npy` reader: little-endian, C-order, 1-D or 2-D, int16/int32.
fn load_npy_i64(path: &Path) -> Vec<i64> {
    let bytes = std::fs::read(path).unwrap();
    assert_eq!(&bytes[..6], b"\x93NUMPY");
    let hlen = u16::from_le_bytes([bytes[8], bytes[9]]) as usize;
    let header = std::str::from_utf8(&bytes[10..10 + hlen]).unwrap();
    let data = &bytes[10 + hlen..];
    if header.contains("'<i2'") || header.contains("'|i2'") {
        data.chunks_exact(2)
            .map(|c| i16::from_le_bytes([c[0], c[1]]) as i64)
            .collect()
    } else if header.contains("'<i4'") {
        data.chunks_exact(4)
            .map(|c| i32::from_le_bytes(c.try_into().unwrap()) as i64)
            .collect()
    } else if header.contains("'<u2'") {
        data.chunks_exact(2)
            .map(|c| u16::from_le_bytes([c[0], c[1]]) as i64)
            .collect()
    } else {
        panic!("unhandled .npy dtype in header: {header}");
    }
}

struct Counting<S> {
    inner: S,
    bytes: AtomicU64,
}
impl<S: ByteSource> Counting<S> {
    fn new(inner: S) -> Self {
        Self {
            inner,
            bytes: AtomicU64::new(0),
        }
    }
}
impl<S: ByteSource> ByteSource for Counting<S> {
    fn len(&self) -> u64 {
        self.inner.len()
    }
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> std::io::Result<()> {
        self.inner.read_exact_at(buf, offset)?;
        self.bytes.fetch_add(buf.len() as u64, Ordering::Relaxed);
        Ok(())
    }
}

#[test]
fn tile_decompression_matches_astropy_reference() {
    let Some(py) = python() else {
        eprintln!("SKIP: no python3+astropy or uv available");
        return;
    };
    let dir = scratch("ref");
    let status = Command::new(&py[0])
        .args(&py[1..])
        .arg("-c")
        .arg(GEN)
        .arg(dir.to_str().unwrap())
        .status()
        .expect("run generator");
    assert!(status.success(), "astropy reference generation failed");

    // Bit-exact whole-image decode for every supported codec.
    for name in [
        "rice16", "rice32", "riceu16", "gzip1_16", "gzip2_16", "gzip1_32", "nocomp16",
    ] {
        let want = load_npy_i64(&dir.join(format!("{name}.npy")));
        let reader = FitsReader::open(dir.join(format!("{name}.fits"))).unwrap();
        let img = reader.compressed_image(1).unwrap();
        let got = img.read_full::<i64>().unwrap();
        assert_eq!(got.len(), want.len(), "{name}: pixel count");
        assert_eq!(got, want, "{name}: decoded pixels differ from astropy");
    }

    // Region read equals the crop of a full read, and only touches
    // intersecting tiles.
    {
        let reader = FitsReader::open(dir.join("rice16.fits")).unwrap();
        let img = reader.compressed_image(1).unwrap();
        assert_eq!(img.shape(), &[60, 40]);
        let full = img.read_full::<i64>().unwrap();
        let region = Region::rect(20, 9, 12, 10);
        let got = img.read_region::<i64>(&region).unwrap();
        let mut want = Vec::new();
        for y in 9..19 {
            for x in 20..32 {
                want.push(full[y * 60 + x]);
            }
        }
        assert_eq!(got, want, "region read != cropped full read");
    }

    {
        let path = dir.join("rice16.fits");
        let file_len = std::fs::metadata(&path).unwrap().len();
        let reader =
            FitsReader::from_source(Counting::new(FileSource::open(&path).unwrap())).unwrap();
        let img = reader.compressed_image(1).unwrap();
        // tiles are 16x8 over a 60x40 image -> 4x5 = 20 tiles. This region
        // sits inside a single tile.
        let before = reader.source().bytes.load(Ordering::Relaxed);
        let _ = img.read_region::<i64>(&Region::rect(2, 2, 4, 4)).unwrap();
        let touched = reader.source().bytes.load(Ordering::Relaxed) - before;
        assert!(
            touched < file_len / 4,
            "single-tile region touched {touched} of {file_len} bytes — not tile-selective"
        );
    }

    // Unsupported types: a typed error, never a panic.
    for (name, needle) in [("plio_mask", "PLIO"), ("hcomp16", "HCOMPRESS")] {
        let reader = FitsReader::open(dir.join(format!("{name}.fits"))).unwrap();
        match reader.compressed_image(1) {
            Err(FitsError::UnsupportedCompression(s)) => {
                assert!(s.contains(needle), "{name}: error mentions {needle:?}: {s}")
            }
            other => panic!("{name}: expected UnsupportedCompression, got {other:?}"),
        }
    }
}

/// P7-T7: our RICE_1 / GZIP_1 compressed writes read back bit-exact — both
/// through our own reader and through astropy.
#[test]
fn compressed_writes_roundtrip_here_and_in_astropy() {
    let dir = scratch("write");

    // Deterministic i16 image with a smooth gradient plus noise.
    let (w, h) = (57usize, 41usize);
    let mut img = vec![0i16; w * h];
    let mut s: u64 = 0xC0FFEE;
    for (i, p) in img.iter_mut().enumerate() {
        s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
        *p = ((i as i64 * 3) % 400 - 200 + ((s >> 56) as i64 - 128) / 8) as i16;
    }

    for (algo, tag) in [(CompAlgo::Rice1, "rice"), (CompAlgo::Gzip1, "gzip")] {
        let path = dir.join(format!("out_{tag}.fits"));
        let builder = CompressedImageBuilder::new(BitPix::I16, &[w, h])
            .unwrap()
            .algorithm(algo)
            .tile_shape(&[16, 12])
            .unwrap()
            .pixels(&img)
            .unwrap();
        let mut fw = FitsWriter::create(&path).unwrap();
        fw.write_image::<u8>(&HeaderBuilder::primary_image(BitPix::U8, &[]).unwrap(), &[])
            .unwrap();
        fw.write_compressed_image(&builder).unwrap();
        fw.finish().unwrap();

        // Our reader.
        let got = FitsReader::open(&path)
            .unwrap()
            .compressed_image(1)
            .unwrap()
            .read_full::<i16>()
            .unwrap();
        assert_eq!(got, img, "{tag}: our read of our write");

        // astropy.
        if let Some(py) = python() {
            let script = format!(
                "import sys, numpy as np\n\
                 from astropy.io import fits\n\
                 d = fits.open(r'{}')[1].data\n\
                 ref = np.load(r'{}')\n\
                 sys.exit(0 if np.array_equal(d, ref) else 1)\n",
                path.display(),
                dir.join(format!("out_{tag}.npy")).display()
            );
            // Dump reference array.
            let dump = format!(
                "import numpy as np\n\
                 a = np.frombuffer(bytes({:?}), dtype='<i2').reshape({h}, {w})\n\
                 np.save(r'{}', a)\n",
                img.iter()
                    .flat_map(|v| v.to_le_bytes())
                    .collect::<Vec<u8>>(),
                dir.join(format!("out_{tag}.npy")).display()
            );
            let ok = Command::new(&py[0])
                .args(&py[1..])
                .arg("-c")
                .arg(&dump)
                .status()
                .unwrap()
                .success();
            assert!(ok, "{tag}: reference dump failed");
            let status = Command::new(&py[0])
                .args(&py[1..])
                .arg("-c")
                .arg(&script)
                .status()
                .unwrap();
            assert!(
                status.success(),
                "{tag}: astropy did not read our write bit-exact"
            );
        }
    }
}
