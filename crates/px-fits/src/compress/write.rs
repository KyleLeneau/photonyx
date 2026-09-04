//! Tile-compressed image *writing* (ADR 006 P7-T7): `RICE_1` and `GZIP_1`
//! only. Produces a `BINTABLE` extension with `ZIMAGE = T`, a
//! `COMPRESSED_DATA` (`1PB`) column holding one compressed tile per row (in
//! row-major tile order), and the `Z*` keywords the reader
//! ([`super::CompressedImageHdu`]) and astropy/`funpack` expect.

use std::io::Write as _;

use flate2::Compression;
use flate2::write::GzEncoder;

use crate::card::Value;
use crate::compress::rice;
use crate::error::FitsError;
use crate::header::BitPix;
use crate::image::Pixel;
use crate::table::{BinTableBuilder, Cell};

/// The compression algorithm for [`CompressedImageBuilder`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CompAlgo {
    Rice1,
    Gzip1,
}

/// Builds a tile-compressed image extension HDU.
#[derive(Debug, Clone)]
pub struct CompressedImageBuilder {
    bitpix: BitPix,
    shape: Vec<usize>,
    tile_shape: Vec<usize>,
    algo: CompAlgo,
    blocksize: usize,
    /// Raw `BITPIX`-representation pixel values, row-major (axis 0 fastest).
    pixels: Vec<i64>,
    extra: Vec<(String, Value, Option<String>)>,
}

impl CompressedImageBuilder {
    /// Starts a builder for an image of `shape` (`NAXIS1..NAXISn`, fastest
    /// axis first) with the default tiling (one full first-axis row per
    /// tile).
    pub fn new(bitpix: BitPix, shape: &[usize]) -> Result<Self, FitsError> {
        if matches!(bitpix, BitPix::F32 | BitPix::F64) {
            return Err(FitsError::UnsupportedCompression(
                "floating-point tile compression is not supported".to_string(),
            ));
        }
        let tile_shape = shape
            .iter()
            .enumerate()
            .map(|(i, &n)| if i == 0 { n.max(1) } else { 1 })
            .collect();
        Ok(Self {
            bitpix,
            shape: shape.to_vec(),
            tile_shape,
            algo: CompAlgo::Rice1,
            blocksize: 32,
            pixels: Vec::new(),
            extra: Vec::new(),
        })
    }

    pub fn algorithm(mut self, algo: CompAlgo) -> Self {
        self.algo = algo;
        self
    }

    pub fn tile_shape(mut self, tile: &[usize]) -> Result<Self, FitsError> {
        if tile.len() != self.shape.len() {
            return Err(FitsError::Processing(
                "tile shape dimensionality does not match the image".to_string(),
            ));
        }
        self.tile_shape = tile.iter().map(|&t| t.max(1)).collect();
        Ok(self)
    }

    /// Sets the pixel data. `pixels.len()` must equal the image's pixel
    /// count; values are the raw `BITPIX` representation (apply `BZERO`
    /// yourself and declare it with [`card`](Self::card) if needed).
    pub fn pixels<T: Pixel>(mut self, pixels: &[T]) -> Result<Self, FitsError> {
        if T::STORAGE_BITPIX != Some(self.bitpix.as_i64()) {
            return Err(FitsError::Processing(format!(
                "pixel type is not the natural storage type for ZBITPIX {}",
                self.bitpix.as_i64()
            )));
        }
        let want: usize = self.shape.iter().product();
        if pixels.len() != want {
            return Err(FitsError::BufferLenMismatch {
                expected: want,
                got: pixels.len(),
            });
        }
        self.pixels = pixels.iter().map(|p| pixel_to_i64(*p)).collect();
        Ok(self)
    }

    /// Appends a header card (e.g. `BZERO`, `EXTNAME`) after the `Z*` block.
    pub fn card(mut self, keyword: &str, value: Value, comment: Option<&str>) -> Self {
        self.extra
            .push((keyword.to_string(), value, comment.map(str::to_string)));
        self
    }

    fn bytepix(&self) -> usize {
        self.bitpix.bytes_per_pixel().max(1)
    }

    fn tiles_per_axis(&self) -> Vec<usize> {
        self.shape
            .iter()
            .zip(&self.tile_shape)
            .map(|(&a, &t)| a.div_ceil(t.max(1)).max(1))
            .collect()
    }

    /// Compresses one tile's pixels to its stored byte stream.
    fn compress_tile(&self, tile_pixels: &[i64]) -> Result<Vec<u8>, FitsError> {
        match self.algo {
            CompAlgo::Rice1 => rice::encode(tile_pixels, self.bytepix(), self.blocksize),
            CompAlgo::Gzip1 => {
                let bpp = self.bytepix();
                let mut raw = Vec::with_capacity(tile_pixels.len() * bpp);
                for &v in tile_pixels {
                    let u = v as u64;
                    for k in (0..bpp).rev() {
                        raw.push((u >> (8 * k)) as u8);
                    }
                }
                let mut enc = GzEncoder::new(Vec::new(), Compression::default());
                enc.write_all(&raw)
                    .and_then(|_| enc.finish())
                    .map_err(|e| FitsError::UnsupportedCompression(format!("GZIP: {e}")))
            }
        }
    }

    /// Gathers a tile's pixels from the row-major image buffer, in tile
    /// element order (axis 0 fastest).
    fn tile_pixels(&self, origin: &[usize], dims: &[usize]) -> Vec<i64> {
        let n = self.shape.len();
        let mut istride = vec![1usize; n];
        for k in 1..n {
            istride[k] = istride[k - 1] * self.shape[k - 1];
        }
        let count: usize = dims.iter().product();
        let mut out = Vec::with_capacity(count);
        let mut local = vec![0usize; n];
        for _ in 0..count {
            let mut pos = 0usize;
            for k in 0..n {
                pos += (origin[k] + local[k]) * istride[k];
            }
            out.push(self.pixels[pos]);
            for k in 0..n {
                local[k] += 1;
                if local[k] < dims[k] {
                    break;
                }
                local[k] = 0;
            }
        }
        out
    }

    /// The complete extension HDU bytes.
    pub(crate) fn serialize(&self) -> Result<Vec<u8>, FitsError> {
        if self.pixels.len() != self.shape.iter().product::<usize>() {
            return Err(FitsError::Processing(
                "compressed image: pixels() was not called with the full image".to_string(),
            ));
        }
        let n = self.shape.len();
        let tpa = self.tiles_per_axis();
        let n_tiles: usize = tpa.iter().product();

        let mut table = BinTableBuilder::new().column("COMPRESSED_DATA", "1PB")?;
        for t in 0..n_tiles {
            let mut coord = vec![0usize; n];
            let mut rem = t;
            for k in 0..n {
                coord[k] = rem % tpa[k];
                rem /= tpa[k];
            }
            let origin: Vec<usize> = (0..n).map(|k| coord[k] * self.tile_shape[k]).collect();
            let dims: Vec<usize> = (0..n)
                .map(|k| self.tile_shape[k].min(self.shape[k] - origin[k]))
                .collect();
            let compressed = self.compress_tile(&self.tile_pixels(&origin, &dims))?;
            table = table.push_row(vec![Cell::Bytes(compressed)])?;
        }

        // Z* keywords, in the order astropy emits them.
        table = table
            .card(
                "ZIMAGE",
                Value::Logical(true),
                Some("extension contains compressed image"),
            )
            .card("ZTENSION", Value::String("IMAGE".into()), None)
            .card("ZBITPIX", Value::Integer(self.bitpix.as_i64()), None)
            .card("ZNAXIS", Value::Integer(n as i64), None);
        for (k, &axis) in self.shape.iter().enumerate() {
            table = table.card(
                &format!("ZNAXIS{}", k + 1),
                Value::Integer(axis as i64),
                None,
            );
        }
        table =
            table
                .card("ZPCOUNT", Value::Integer(0), None)
                .card("ZGCOUNT", Value::Integer(1), None);
        for (k, &t) in self.tile_shape.iter().enumerate() {
            table = table.card(&format!("ZTILE{}", k + 1), Value::Integer(t as i64), None);
        }
        table = table.card(
            "ZCMPTYPE",
            Value::String(match self.algo {
                CompAlgo::Rice1 => "RICE_1".into(),
                CompAlgo::Gzip1 => "GZIP_1".into(),
            }),
            None,
        );
        if self.algo == CompAlgo::Rice1 {
            table = table
                .card("ZNAME1", Value::String("BLOCKSIZE".into()), None)
                .card("ZVAL1", Value::Integer(self.blocksize as i64), None)
                .card("ZNAME2", Value::String("BYTEPIX".into()), None)
                .card("ZVAL2", Value::Integer(self.bytepix() as i64), None);
        }
        table = table.card(
            "EXTNAME",
            Value::String("COMPRESSED_IMAGE".into()),
            Some("name of this binary table extension"),
        );
        for (kw, v, c) in &self.extra {
            table = table.card(kw, v.clone(), c.as_deref());
        }

        table.serialize()
    }
}

fn pixel_to_i64<T: Pixel>(_p: T) -> i64 {
    // `Pixel` has no "to integer" method; go via a byte encode + decode.
    let mut buf = [0u8; 8];
    _p.encode_be(&mut buf);
    let w = std::mem::size_of::<T>();
    match w {
        1 => buf[0] as i64,
        2 => i16::from_be_bytes([buf[0], buf[1]]) as i64,
        4 => i32::from_be_bytes([buf[0], buf[1], buf[2], buf[3]]) as i64,
        8 => i64::from_be_bytes(buf),
        _ => 0,
    }
}
