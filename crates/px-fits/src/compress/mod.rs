//! Tile-compressed images — the FITS tiled-image convention (ADR 006 Phase 7,
//! <https://fits.gsfc.nasa.gov/registry/tilecompression.html>).
//!
//! A compressed image is a `BINTABLE` extension carrying `ZIMAGE = T`. The
//! logical image's geometry lives in `ZBITPIX`/`ZNAXIS`/`ZNAXISn`; it is cut
//! into tiles of `ZTILEn` pixels, each tile compressed independently with
//! `ZCMPTYPE` and stored (in row-major tile order) as one variable-length
//! byte array in the `COMPRESSED_DATA` column. [`CompressedImageHdu`]
//! presents all of this as an ordinary image: [`shape`](CompressedImageHdu::shape),
//! [`read_full`](CompressedImageHdu::read_full), and
//! [`read_region`](CompressedImageHdu::read_region), the last decompressing
//! only the tiles the region intersects (P7-T6).
//!
//! Supported `ZCMPTYPE`: `RICE_1`, `GZIP_1`, `GZIP_2`, `NOCOMPRESS`.
//! `PLIO_1`, `HCOMPRESS_1`, and any unknown type return
//! [`FitsError::UnsupportedCompression`] — never a panic. Floating-point
//! `ZBITPIX` (per-tile quantization) is likewise not yet supported.

pub mod rice;

use std::io::Read;

use flate2::read::MultiGzDecoder;

use crate::error::FitsError;
use crate::hdu::DiscoveredHdu;
use crate::header::{BitPix, Header};
use crate::image::{Pixel, Region, Scaling};
use crate::source::ByteSource;
use crate::table::BinTableHdu;
use crate::table::column_index;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CmpType {
    Rice1,
    Gzip1,
    Gzip2,
    NoCompress,
}

impl CmpType {
    fn parse(raw: &str) -> Result<CmpType, FitsError> {
        match raw.trim() {
            "RICE_1" | "RICE_ONE" => Ok(CmpType::Rice1),
            "GZIP_1" => Ok(CmpType::Gzip1),
            "GZIP_2" => Ok(CmpType::Gzip2),
            "NOCOMPRESS" => Ok(CmpType::NoCompress),
            other => Err(FitsError::UnsupportedCompression(other.to_string())),
        }
    }
}

/// A tile-compressed image HDU, presented as an image.
#[derive(Debug)]
pub struct CompressedImageHdu<'a, S: ByteSource + ?Sized> {
    table: BinTableHdu<'a, S>,
    bitpix: BitPix,
    /// `ZNAXISn`, fastest-varying axis first.
    shape: Vec<usize>,
    /// `ZTILEn`.
    tile_shape: Vec<usize>,
    /// Number of tiles along each axis.
    tiles_per_axis: Vec<usize>,
    cmptype: CmpType,
    /// `RICE_1` `BLOCKSIZE` / `BYTEPIX`.
    rice_blocksize: usize,
    rice_bytepix: usize,
    scaling: Scaling,
    compressed_col: usize,
    gzip_fallback_col: Option<usize>,
    uncompressed_col: Option<usize>,
}

impl<'a, S: ByteSource + ?Sized> CompressedImageHdu<'a, S> {
    pub(crate) fn from_discovered(source: &'a S, hdu: DiscoveredHdu) -> Result<Self, FitsError> {
        let header = hdu.header.clone();
        if !header.get_bool("ZIMAGE").unwrap_or(false) {
            return Err(FitsError::Processing(
                "HDU is a BINTABLE but not a tile-compressed image (no ZIMAGE = T)".to_string(),
            ));
        }

        let bitpix = BitPix::from_i64(header.get_i64("ZBITPIX").ok_or_else(|| {
            FitsError::Processing("compressed image has no ZBITPIX".to_string())
        })?)?;
        if matches!(bitpix, BitPix::F32 | BitPix::F64) {
            return Err(FitsError::UnsupportedCompression(
                "floating-point tile compression (per-tile quantization) is not supported"
                    .to_string(),
            ));
        }

        let znaxis = header
            .get_i64("ZNAXIS")
            .and_then(|v| usize::try_from(v).ok())
            .ok_or_else(|| FitsError::Processing("compressed image has no ZNAXIS".to_string()))?;

        let mut shape = Vec::with_capacity(znaxis);
        let mut tile_shape = Vec::with_capacity(znaxis);
        for n in 1..=znaxis {
            let axis = header
                .get_i64(&format!("ZNAXIS{n}"))
                .and_then(|v| usize::try_from(v).ok())
                .ok_or_else(|| FitsError::Processing(format!("missing ZNAXIS{n}")))?;
            // Default tiling is one full row of the first axis, 1 on the rest.
            let default_tile = if n == 1 { axis } else { 1 };
            let tile = header
                .get_i64(&format!("ZTILE{n}"))
                .and_then(|v| usize::try_from(v).ok())
                .unwrap_or(default_tile)
                .max(1)
                .min(axis.max(1));
            shape.push(axis);
            tile_shape.push(tile);
        }

        let tiles_per_axis: Vec<usize> = shape
            .iter()
            .zip(&tile_shape)
            .map(|(&a, &t)| a.div_ceil(t.max(1)).max(1))
            .collect();

        let cmptype = CmpType::parse(&header.get_string("ZCMPTYPE").ok_or_else(|| {
            FitsError::Processing("compressed image has no ZCMPTYPE".to_string())
        })?)?;

        // RICE parameters from ZNAMEn/ZVALn.
        let mut rice_blocksize = 32usize;
        let mut rice_bytepix = (bitpix.bytes_per_pixel()).max(1);
        let mut n = 1;
        while let Some(name) = header.get_string(&format!("ZNAME{n}")) {
            let val = header.get_i64(&format!("ZVAL{n}"));
            match name.trim() {
                "BLOCKSIZE" => {
                    if let Some(v) = val.and_then(|v| usize::try_from(v).ok()) {
                        rice_blocksize = v;
                    }
                }
                "BYTEPIX" => {
                    if let Some(v) = val.and_then(|v| usize::try_from(v).ok()) {
                        rice_bytepix = v;
                    }
                }
                _ => {}
            }
            n += 1;
        }

        let table = BinTableHdu::from_discovered(source, hdu)?;
        let compressed_col = column_index(table.columns(), "COMPRESSED_DATA").ok_or_else(|| {
            FitsError::Processing("compressed image has no COMPRESSED_DATA column".to_string())
        })?;
        let gzip_fallback_col = column_index(table.columns(), "GZIP_COMPRESSED_DATA");
        let uncompressed_col = column_index(table.columns(), "UNCOMPRESSED_DATA");

        Ok(Self {
            bitpix,
            shape,
            tile_shape,
            tiles_per_axis,
            cmptype,
            rice_blocksize,
            rice_bytepix,
            scaling: Scaling::from_header(&header),
            compressed_col,
            gzip_fallback_col,
            uncompressed_col,
            table,
        })
    }

    /// The logical image header (the compressed-HDU header — mandatory image
    /// keywords appear `Z`-prefixed, the rest verbatim).
    pub fn header(&self) -> &Header {
        self.table.header()
    }

    /// `ZNAXIS1..ZNAXISn`, fastest-varying axis first.
    pub fn shape(&self) -> &[usize] {
        &self.shape
    }

    /// The logical image `BITPIX` (`ZBITPIX`).
    pub fn bitpix(&self) -> BitPix {
        self.bitpix
    }

    pub fn tile_shape(&self) -> &[usize] {
        &self.tile_shape
    }

    pub fn tile_count(&self) -> usize {
        self.tiles_per_axis.iter().product()
    }

    fn pixel_count(&self) -> usize {
        if self.shape.is_empty() {
            0
        } else {
            self.shape.iter().product()
        }
    }

    /// Decompresses the whole image into a freshly allocated `Vec<T>`,
    /// applying `BSCALE`/`BZERO`/`BLANK`.
    pub fn read_full<T: Pixel>(&self) -> Result<Vec<T>, FitsError> {
        let mut out = vec![T::from_i64(0); self.pixel_count()];
        for t in 0..self.tile_count() {
            let geom = self.tile_geometry(t);
            let raw = self.decode_tile(t, geom.pixels)?;
            self.scatter_tile(&raw, &geom, &self.shape, &mut out);
        }
        Ok(out)
    }

    /// Decompresses only the tiles that intersect `region`, into a `Vec<T>`
    /// laid out like the corresponding slice of [`read_full`](Self::read_full).
    pub fn read_region<T: Pixel>(&self, region: &Region) -> Result<Vec<T>, FitsError> {
        region.validate(&self.shape)?;
        let mut out = vec![T::from_i64(0); region.len()];
        if out.is_empty() {
            return Ok(out);
        }
        for t in self.tiles_intersecting(region) {
            let geom = self.tile_geometry(t);
            let raw = self.decode_tile(t, geom.pixels)?;
            self.scatter_tile_into_region(&raw, &geom, region, &mut out);
        }
        Ok(out)
    }

    // --- tile geometry ----------------------------------------------------

    #[allow(clippy::needless_range_loop)] // parallel per-axis index vecs
    fn tile_geometry(&self, tile: usize) -> TileGeometry {
        let n = self.shape.len();
        let mut coord = vec![0usize; n];
        let mut rem = tile;
        for k in 0..n {
            coord[k] = rem % self.tiles_per_axis[k];
            rem /= self.tiles_per_axis[k];
        }
        let mut origin = vec![0usize; n];
        let mut dims = vec![0usize; n];
        for k in 0..n {
            origin[k] = coord[k] * self.tile_shape[k];
            dims[k] = self.tile_shape[k].min(self.shape[k] - origin[k]);
        }
        let pixels = dims.iter().product();
        TileGeometry {
            origin,
            dims,
            pixels,
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn tiles_intersecting(&self, region: &Region) -> Vec<usize> {
        let n = self.shape.len();
        // Per-axis inclusive tile-coordinate range the region covers.
        let ranges: Vec<(usize, usize)> = (0..n)
            .map(|k| {
                let lo = region.start()[k] / self.tile_shape[k];
                let hi = (region.start()[k] + region.shape()[k] - 1) / self.tile_shape[k];
                (lo, hi)
            })
            .collect();

        let mut tiles = Vec::new();
        let mut coord: Vec<usize> = ranges.iter().map(|&(lo, _)| lo).collect();
        loop {
            // Linear tile index (axis 0 fastest).
            let mut idx = 0usize;
            let mut stride = 1usize;
            for k in 0..n {
                idx += coord[k] * stride;
                stride *= self.tiles_per_axis[k];
            }
            tiles.push(idx);

            let mut k = 0;
            loop {
                if k == n {
                    return tiles;
                }
                coord[k] += 1;
                if coord[k] <= ranges[k].1 {
                    break;
                }
                coord[k] = ranges[k].0;
                k += 1;
            }
        }
    }

    // --- decompression --------------------------------------------------

    fn decode_tile(&self, tile: usize, npix: usize) -> Result<Vec<i64>, FitsError> {
        let bytes = self.table.var_raw_bytes(self.compressed_col, tile)?;

        // A tile that didn't compress well falls back to a per-tile GZIP_1
        // stream (or verbatim UNCOMPRESSED_DATA).
        if bytes.is_empty() {
            if let Some(col) = self.gzip_fallback_col {
                let g = self.table.var_raw_bytes(col, tile)?;
                if !g.is_empty() {
                    return self.gunzip_to_ints(&g, npix, false);
                }
            }
            if let Some(col) = self.uncompressed_col {
                let u = self.table.var_raw_bytes(col, tile)?;
                if !u.is_empty() {
                    return Ok(be_ints(&u, npix, self.bitpix));
                }
            }
        }

        match self.cmptype {
            CmpType::Rice1 => rice::decode(&bytes, npix, self.rice_bytepix, self.rice_blocksize),
            CmpType::Gzip1 => self.gunzip_to_ints(&bytes, npix, false),
            CmpType::Gzip2 => self.gunzip_to_ints(&bytes, npix, true),
            CmpType::NoCompress => Ok(be_ints(&bytes, npix, self.bitpix)),
        }
    }

    fn gunzip_to_ints(
        &self,
        gz: &[u8],
        npix: usize,
        shuffled: bool,
    ) -> Result<Vec<i64>, FitsError> {
        let bpp = self.bitpix.bytes_per_pixel();
        let mut raw = Vec::with_capacity(npix * bpp);
        MultiGzDecoder::new(gz)
            .read_to_end(&mut raw)
            .map_err(|e| FitsError::UnsupportedCompression(format!("GZIP: {e}")))?;
        if raw.len() < npix * bpp {
            return Err(FitsError::UnsupportedCompression(
                "GZIP tile decompressed to fewer bytes than the tile holds".to_string(),
            ));
        }
        if shuffled {
            // GZIP_2 stores byte plane j (0 = most significant) contiguously.
            let mut unshuffled = vec![0u8; npix * bpp];
            for j in 0..bpp {
                let plane = &raw[j * npix..(j + 1) * npix];
                for (p, &b) in plane.iter().enumerate() {
                    unshuffled[p * bpp + j] = b;
                }
            }
            Ok(be_ints(&unshuffled, npix, self.bitpix))
        } else {
            Ok(be_ints(&raw, npix, self.bitpix))
        }
    }

    // --- scatter -------------------------------------------------------

    fn scatter_tile<T: Pixel>(
        &self,
        raw: &[i64],
        geom: &TileGeometry,
        image_shape: &[usize],
        out: &mut [T],
    ) {
        for (pos, &v) in self.tile_pixel_positions(geom, image_shape).zip(raw) {
            out[pos] = self.apply_scaling::<T>(v);
        }
    }

    #[allow(clippy::needless_range_loop)]
    fn scatter_tile_into_region<T: Pixel>(
        &self,
        raw: &[i64],
        geom: &TileGeometry,
        region: &Region,
        out: &mut [T],
    ) {
        let n = self.shape.len();
        // Strides of the region-shaped output.
        let mut rstride = vec![1usize; n];
        for k in 1..n {
            rstride[k] = rstride[k - 1] * region.shape()[k - 1];
        }

        let mut local = vec![0usize; n]; // index within the tile
        for &v in raw {
            // Global pixel coordinate for this tile element.
            let mut inside = true;
            let mut roff = 0usize;
            for k in 0..n {
                let g = geom.origin[k] + local[k];
                if g < region.start()[k] || g >= region.start()[k] + region.shape()[k] {
                    inside = false;
                    break;
                }
                roff += (g - region.start()[k]) * rstride[k];
            }
            if inside {
                out[roff] = self.apply_scaling::<T>(v);
            }

            // Advance the tile multi-index, axis 0 fastest.
            for k in 0..n {
                local[k] += 1;
                if local[k] < geom.dims[k] {
                    break;
                }
                local[k] = 0;
            }
        }
    }

    /// Positions in the full image buffer for each tile element, in tile
    /// element order (axis 0 fastest).
    #[allow(clippy::needless_range_loop)]
    fn tile_pixel_positions<'g>(
        &'g self,
        geom: &'g TileGeometry,
        image_shape: &'g [usize],
    ) -> impl Iterator<Item = usize> + 'g {
        let n = image_shape.len();
        let mut istride = vec![1usize; n];
        for k in 1..n {
            istride[k] = istride[k - 1] * image_shape[k - 1];
        }
        let mut local = vec![0usize; n];
        let mut done = geom.pixels == 0;
        std::iter::from_fn(move || {
            if done {
                return None;
            }
            let mut pos = 0usize;
            for k in 0..n {
                pos += (geom.origin[k] + local[k]) * istride[k];
            }
            for k in 0..n {
                local[k] += 1;
                if local[k] < geom.dims[k] {
                    return Some(pos);
                }
                local[k] = 0;
            }
            done = true;
            Some(pos)
        })
    }

    fn apply_scaling<T: Pixel>(&self, raw: i64) -> T {
        if Some(raw) == self.scaling.blank {
            return if T::IS_FLOAT {
                T::from_f64(f64::NAN)
            } else {
                T::from_i64(raw)
            };
        }
        if let Some(z) = self.scaling.int_offset() {
            T::from_i64(raw.wrapping_add(z))
        } else if self.scaling.is_identity() {
            T::from_i64(raw)
        } else {
            T::from_phys(self.scaling.bzero + self.scaling.bscale * raw as f64)
        }
    }
}

struct TileGeometry {
    origin: Vec<usize>,
    dims: Vec<usize>,
    pixels: usize,
}

/// Interprets `bytes` as `npix` big-endian `|BITPIX|/8`-byte integers, sign
/// extended per `bitpix` (`BITPIX = 8` is unsigned).
fn be_ints(bytes: &[u8], npix: usize, bitpix: BitPix) -> Vec<i64> {
    let bpp = bitpix.bytes_per_pixel();
    bytes
        .chunks_exact(bpp)
        .take(npix)
        .map(|c| match bitpix {
            BitPix::U8 => c[0] as i64,
            BitPix::I16 => i16::from_be_bytes(c.try_into().unwrap()) as i64,
            BitPix::I32 => i32::from_be_bytes(c.try_into().unwrap()) as i64,
            BitPix::I64 => i64::from_be_bytes(c.try_into().unwrap()),
            BitPix::F32 => f32::from_be_bytes(c.try_into().unwrap()) as i64,
            BitPix::F64 => f64::from_be_bytes(c.try_into().unwrap()) as i64,
        })
        .collect()
}
