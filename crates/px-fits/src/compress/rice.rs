//! `RICE_1` tile decompression (ADR 006 P7-T2).
//!
//! A direct port of cfitsio's `fits_rdecomp` family (`ricecomp.c`): the tile
//! is stored as the first pixel value verbatim (`bytepix` big-endian bytes)
//! followed by Rice-coded successive differences, in blocks of `blocksize`
//! pixels that each begin with an `fsbits`-wide split parameter. Three block
//! modes: all-zero differences, raw `bbits`-wide values, and the normal
//! unary-quotient + `fs`-bit-remainder Rice code. Differences are
//! zigzag-mapped to non-negative integers.

use crate::error::FitsError;

/// Decodes `nx` pixels from a `RICE_1` byte stream. `bytepix` is 1, 2, or 4;
/// `blocksize` is the Rice block length (`BLOCKSIZE`, commonly 32). The
/// returned values are the raw `ZBITPIX`-representation integers, sign
/// extended per `bytepix`.
pub fn decode(
    src: &[u8],
    nx: usize,
    bytepix: usize,
    blocksize: usize,
) -> Result<Vec<i64>, FitsError> {
    let (fsbits, fsmax): (i32, i32) = match bytepix {
        1 => (3, 6),
        2 => (4, 14),
        4 => (5, 25),
        other => {
            return Err(FitsError::UnsupportedCompression(format!(
                "RICE_1 BYTEPIX={other} (only 1, 2, 4 are supported)"
            )));
        }
    };
    let bbits = 1i32 << fsbits;
    if blocksize == 0 {
        return Err(FitsError::UnsupportedCompression(
            "RICE_1 BLOCKSIZE=0".to_string(),
        ));
    }

    let mut out = vec![0i64; nx];
    if nx == 0 {
        return Ok(out);
    }
    if src.len() < bytepix {
        return Err(FitsError::UnsupportedCompression(
            "RICE_1 tile shorter than one pixel".to_string(),
        ));
    }

    // First pixel: bytepix big-endian bytes.
    let mut lastpix: u64 = 0;
    for &b in &src[..bytepix] {
        lastpix = (lastpix << 8) | b as u64;
    }

    let mut reader = BitReader::new(&src[bytepix..]);
    let mask = width_mask(bytepix);

    let mut i = 0usize;
    while i < nx {
        let raw_fs = reader.get_bits(fsbits)?;
        let fs = raw_fs as i32 - 1;
        let imax = (i + blocksize).min(nx);

        if fs < 0 {
            // Low entropy: every difference in the block is zero.
            let val = sign_extend(lastpix, bytepix);
            out[i..imax].fill(val);
        } else if fs == fsmax {
            // High entropy: raw bbits-wide values.
            for slot in out[i..imax].iter_mut() {
                let diff = reader.get_bits(bbits)? as u64;
                lastpix = apply_diff(lastpix, diff, mask);
                *slot = sign_extend(lastpix, bytepix);
            }
        } else {
            for slot in out[i..imax].iter_mut() {
                let nzero = reader.count_zeros()?;
                let low = if fs > 0 {
                    reader.get_bits(fs)? as u64
                } else {
                    0
                };
                let diff = ((nzero as u64) << fs) | low;
                lastpix = apply_diff(lastpix, diff, mask);
                *slot = sign_extend(lastpix, bytepix);
            }
        }
        i = imax;
    }
    Ok(out)
}

/// Rice parameters for a given `bytepix` (1, 2, 4).
fn params(bytepix: usize) -> Result<(i32, i32), FitsError> {
    match bytepix {
        1 => Ok((3, 6)),
        2 => Ok((4, 14)),
        4 => Ok((5, 25)),
        other => Err(FitsError::UnsupportedCompression(format!(
            "RICE_1 BYTEPIX={other} (only 1, 2, 4 are supported)"
        ))),
    }
}

/// Encodes `pixels` (raw `ZBITPIX`-representation integers) as a `RICE_1`
/// tile stream, the inverse of [`decode`]. Ported from cfitsio's
/// `fits_rcomp`.
pub fn encode(pixels: &[i64], bytepix: usize, blocksize: usize) -> Result<Vec<u8>, FitsError> {
    let (fsbits, fsmax) = params(bytepix)?;
    let bbits = 1i32 << fsbits;
    if blocksize == 0 {
        return Err(FitsError::UnsupportedCompression(
            "RICE_1 BLOCKSIZE=0".to_string(),
        ));
    }
    let mask = width_mask(bytepix);

    let mut out = Vec::new();
    if pixels.is_empty() {
        return Ok(out);
    }

    // First pixel verbatim, big-endian.
    let first = pixels[0] as u64 & mask;
    for k in (0..bytepix).rev() {
        out.push((first >> (8 * k)) as u8);
    }

    let mut w = BitWriter::new();
    let mut lastpix = first;
    let width = 8 * bytepix as u32;

    let mut i = 0usize;
    let mut diffs = vec![0u64; blocksize];
    while i < pixels.len() {
        let this = (pixels.len() - i).min(blocksize);
        let mut psum = 0u64;
        for (j, slot) in diffs[..this].iter_mut().enumerate() {
            let cur = pixels[i + j] as u64 & mask;
            let d = cur.wrapping_sub(lastpix) & mask;
            let signed = to_signed(d, width);
            let mapped = if signed < 0 {
                !((signed as u64) << 1)
            } else {
                (signed as u64) << 1
            } & mask;
            *slot = mapped;
            psum += mapped;
            lastpix = cur;
        }
        let block = &diffs[..this];

        let mut fs = if psum == 0 {
            -1i32
        } else {
            let mean = (psum - (this as u64) / 2 - 1) / this as u64;
            let mut p = mean >> 1;
            let mut f = 0i32;
            while p > 0 {
                p >>= 1;
                f += 1;
            }
            f
        };
        // Guard against a poor estimate producing pathological unary runs.
        if fs >= 0 && fs < fsmax {
            let unary: u64 = block.iter().map(|&d| d >> fs).sum();
            if unary > (this as u64) * bbits as u64 {
                fs = fsmax;
            }
        }

        if fs < 0 {
            w.put_bits(fsbits as u32, 0);
        } else if fs >= fsmax {
            w.put_bits(fsbits as u32, (fsmax + 1) as u32);
            for &d in block {
                w.put_bits(bbits as u32, d as u32);
            }
        } else {
            w.put_bits(fsbits as u32, (fs + 1) as u32);
            for &d in block {
                w.put_unary(d >> fs);
                if fs > 0 {
                    w.put_bits(fs as u32, (d & ((1u64 << fs) - 1)) as u32);
                }
            }
        }
        i += this;
    }

    out.extend(w.finish());
    Ok(out)
}

/// Two's-complement interpretation of `d` at bit width `width`.
fn to_signed(d: u64, width: u32) -> i64 {
    let half = 1u64 << (width - 1);
    if d & half != 0 {
        d as i64 - (1i64 << width)
    } else {
        d as i64
    }
}

/// MSB-first bit writer flushing to a byte vec.
struct BitWriter {
    out: Vec<u8>,
    acc: u64,
    nbits: u32,
}

impl BitWriter {
    fn new() -> Self {
        Self {
            out: Vec::new(),
            acc: 0,
            nbits: 0,
        }
    }

    fn put_bits(&mut self, n: u32, v: u32) {
        if n == 0 {
            return;
        }
        let masked = if n >= 32 {
            v as u64
        } else {
            (v as u64) & ((1u64 << n) - 1)
        };
        self.acc = (self.acc << n) | masked;
        self.nbits += n;
        while self.nbits >= 8 {
            self.nbits -= 8;
            self.out.push((self.acc >> self.nbits) as u8);
        }
    }

    /// `k` zero bits followed by a single `1` bit.
    fn put_unary(&mut self, k: u64) {
        let mut left = k;
        while left >= 24 {
            self.put_bits(24, 0);
            left -= 24;
        }
        self.put_bits(left as u32 + 1, 1);
    }

    fn finish(mut self) -> Vec<u8> {
        if self.nbits > 0 {
            self.out.push((self.acc << (8 - self.nbits)) as u8);
        }
        self.out
    }
}

fn width_mask(bytepix: usize) -> u64 {
    match bytepix {
        1 => 0xFF,
        2 => 0xFFFF,
        4 => 0xFFFF_FFFF,
        _ => u64::MAX,
    }
}

/// Undoes the zigzag mapping and adds the difference to `lastpix`, wrapping
/// at the `bytepix` width.
fn apply_diff(lastpix: u64, diff: u64, mask: u64) -> u64 {
    let signed_diff = if diff & 1 != 0 {
        // odd -> negative: ~(diff >> 1)
        (!(diff >> 1)) & mask
    } else {
        (diff >> 1) & mask
    };
    lastpix.wrapping_add(signed_diff) & mask
}

/// Interprets the raw `bytepix`-wide value as its `ZBITPIX` integer: `BITPIX
/// = 8` (`bytepix` 1) is unsigned; 16 and 32 are signed two's complement.
fn sign_extend(v: u64, bytepix: usize) -> i64 {
    match bytepix {
        1 => (v & 0xFF) as i64,
        2 => v as u16 as i16 as i64,
        4 => v as u32 as i32 as i64,
        _ => v as i64,
    }
}

/// MSB-first bit reader over a byte slice.
struct BitReader<'a> {
    bytes: &'a [u8],
    pos: usize,
    /// Bit accumulator holding `nbits` unconsumed low bits.
    acc: u32,
    nbits: i32,
}

impl<'a> BitReader<'a> {
    fn new(bytes: &'a [u8]) -> Self {
        Self {
            bytes,
            pos: 0,
            acc: 0,
            nbits: 0,
        }
    }

    fn refill_byte(&mut self) -> Result<(), FitsError> {
        let b = *self.bytes.get(self.pos).ok_or_else(|| {
            FitsError::UnsupportedCompression("RICE_1 stream truncated".to_string())
        })?;
        self.pos += 1;
        self.acc = (self.acc << 8) | b as u32;
        self.nbits += 8;
        Ok(())
    }

    /// Reads `n` bits (0 ≤ n ≤ 25) MSB-first.
    fn get_bits(&mut self, n: i32) -> Result<u32, FitsError> {
        if n == 0 {
            return Ok(0);
        }
        while self.nbits < n {
            self.refill_byte()?;
        }
        self.nbits -= n;
        let v = (self.acc >> self.nbits) & ((1u32 << n) - 1);
        Ok(v)
    }

    /// Counts consecutive `0` bits up to and consuming the terminating `1`.
    fn count_zeros(&mut self) -> Result<u32, FitsError> {
        let mut zeros = 0u32;
        loop {
            if self.nbits == 0 {
                self.refill_byte()?;
            }
            // Peek the top unconsumed bit.
            self.nbits -= 1;
            let bit = (self.acc >> self.nbits) & 1;
            if bit == 1 {
                return Ok(zeros);
            }
            zeros += 1;
            if zeros > 1_000_000 {
                return Err(FitsError::UnsupportedCompression(
                    "RICE_1 runaway unary code".to_string(),
                ));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_pixels_is_empty() {
        assert_eq!(decode(&[], 0, 2, 32).unwrap(), Vec::<i64>::new());
    }

    #[test]
    fn all_zero_diff_block_repeats_the_first_pixel() {
        // bytepix=1 -> fsbits=3. First pixel = 0x40. Then one fs field of
        // value 0 (encodes fs = -1, the all-zero-difference block), so every
        // pixel in the block equals the first.
        // fs byte: bits `000` then padding zeros -> 0x00.
        let src = [0x40u8, 0x00];
        let out = decode(&src, 4, 1, 32).unwrap();
        assert_eq!(out, vec![0x40, 0x40, 0x40, 0x40]);
    }

    #[test]
    fn truncated_stream_errors_without_panicking() {
        let err = decode(&[0x00], 8, 2, 32).unwrap_err();
        assert!(matches!(err, FitsError::UnsupportedCompression(_)));
    }

    fn roundtrip(pixels: &[i64], bytepix: usize, blocksize: usize) {
        let enc = encode(pixels, bytepix, blocksize).unwrap();
        let dec = decode(&enc, pixels.len(), bytepix, blocksize).unwrap();
        assert_eq!(dec, pixels, "bytepix={bytepix} blocksize={blocksize}");
    }

    #[test]
    fn encode_decode_roundtrip_across_shapes_and_widths() {
        // Smooth ramp (low entropy), constant run (all-zero blocks), and a
        // noisy sequence (forces high-entropy blocks).
        let ramp: Vec<i64> = (0..200).map(|i| (i * 3 - 100) as i64).collect();
        let flat: Vec<i64> = vec![7; 150];
        let mut noisy = Vec::new();
        let mut s: u64 = 0x1234_5678;
        for _ in 0..300 {
            s = s.wrapping_mul(6364136223846793005).wrapping_add(1);
            noisy.push(((s >> 40) as i16) as i64);
        }

        for &bs in &[4usize, 16, 32] {
            roundtrip(&ramp, 2, bs);
            roundtrip(&flat, 2, bs);
            roundtrip(&noisy, 2, bs);
            roundtrip(&ramp, 4, bs);
        }
        roundtrip(&[42], 1, 32);
        roundtrip(
            &(0..64).map(|i| (i % 256) as i64).collect::<Vec<_>>(),
            1,
            32,
        );
    }
}
