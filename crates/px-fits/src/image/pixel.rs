//! Pixel decoding: turning a run of raw big-endian on-disk bytes into a
//! caller-chosen numeric type, applying `BSCALE`/`BZERO`/`BLANK` on the way
//! (FITS Standard 4.0 §4.4.1.1 "BITPIX", §4.4.2.5 "BSCALE and BZERO",
//! §4.4.2.6 "BLANK").
//!
//! ADR 006 D2: decoding is `from_be_bytes` over `chunks_exact(N)` — no
//! `unsafe`, no transmute. ADR 006 D4: the caller owns the output buffer and
//! this fills it in place; nothing here allocates.

use crate::error::FitsError;
use crate::header::BitPix;
use crate::image::scaling::Scaling;

mod sealed {
    pub trait Sealed {}
}

/// A numeric type [`crate::image::ImageHdu`] can decode pixels into. Sealed:
/// the set is exactly the eight types the FITS data model needs — the six
/// `BITPIX` types plus `u16`/`u32` for the `BZERO`-encoded unsigned
/// conventions (ADR 006 D6).
pub trait Pixel: Copy + Send + Sync + 'static + sealed::Sealed {
    /// Whether this is a floating-point type. Decoding branches on it so that
    /// scaled integer output truncates toward zero (matching cfitsio) while
    /// scaled float output keeps the fractional part.
    const IS_FLOAT: bool;

    /// Narrowing/typed conversion from an already-computed integer physical
    /// value. `as` casts here are saturating (Rust 1.45+), so an
    /// out-of-range physical value clamps rather than wrapping.
    fn from_i64(v: i64) -> Self;

    /// Conversion from an already-computed floating physical value.
    fn from_f64(v: f64) -> Self;

    /// Route a physical value computed as `f64` to the right conversion.
    #[inline]
    fn from_phys(v: f64) -> Self {
        if Self::IS_FLOAT {
            Self::from_f64(v)
        } else {
            Self::from_i64(v as i64)
        }
    }

    /// The `BITPIX` value for which this type is the *natural on-disk storage
    /// type*, or `None` for `u16`/`u32` (which only exist on read as the
    /// `BZERO`-encoded unsigned conventions, never as a storage type). The
    /// writer uses this to reject a `BITPIX`/`T` mismatch (ADR 006 Phase 5).
    const STORAGE_BITPIX: Option<i64>;

    /// Writes `self` as `size_of::<Self>()` big-endian bytes into the start
    /// of `out` (FITS Standard 4.0 §3.3.1).
    fn encode_be(self, out: &mut [u8]);
}

macro_rules! int_pixel {
    ($t:ty, $storage:expr) => {
        impl sealed::Sealed for $t {}
        impl Pixel for $t {
            const IS_FLOAT: bool = false;
            const STORAGE_BITPIX: Option<i64> = $storage;
            #[inline]
            fn from_i64(v: i64) -> Self {
                v as $t
            }
            #[inline]
            fn from_f64(v: f64) -> Self {
                v as $t
            }
            #[inline]
            fn encode_be(self, out: &mut [u8]) {
                out[..std::mem::size_of::<$t>()].copy_from_slice(&self.to_be_bytes());
            }
        }
    };
}

macro_rules! float_pixel {
    ($t:ty, $storage:expr) => {
        impl sealed::Sealed for $t {}
        impl Pixel for $t {
            const IS_FLOAT: bool = true;
            const STORAGE_BITPIX: Option<i64> = Some($storage);
            #[inline]
            fn from_i64(v: i64) -> Self {
                v as $t
            }
            #[inline]
            fn from_f64(v: f64) -> Self {
                v as $t
            }
            #[inline]
            fn encode_be(self, out: &mut [u8]) {
                out[..std::mem::size_of::<$t>()].copy_from_slice(&self.to_be_bytes());
            }
        }
    };
}

int_pixel!(u8, Some(8));
int_pixel!(i16, Some(16));
int_pixel!(u16, None);
int_pixel!(i32, Some(32));
int_pixel!(u32, None);
int_pixel!(i64, Some(64));
float_pixel!(f32, -32);
float_pixel!(f64, -64);

/// One raw on-disk sample type, i.e. the wire representation a given
/// `BITPIX` value selects. Big-endian per FITS Standard 4.0 §3.3.1.
trait RawSample: Copy {
    const NBYTES: usize;
    fn from_be(bytes: &[u8]) -> Self;
    fn to_i64(self) -> i64;
    fn to_f64(self) -> f64;
}

macro_rules! raw_sample {
    ($t:ty) => {
        impl RawSample for $t {
            const NBYTES: usize = std::mem::size_of::<$t>();
            #[inline]
            fn from_be(bytes: &[u8]) -> Self {
                <$t>::from_be_bytes(bytes.try_into().expect("chunk width == NBYTES"))
            }
            #[inline]
            fn to_i64(self) -> i64 {
                self as i64
            }
            #[inline]
            fn to_f64(self) -> f64 {
                self as f64
            }
        }
    };
}

raw_sample!(u8);
raw_sample!(i16);
raw_sample!(i32);
raw_sample!(i64);
raw_sample!(f32);
raw_sample!(f64);

/// Decode `src` (raw big-endian bytes for `bitpix`) into `dst`, applying
/// `scaling`. `src.len()` must be exactly `dst.len() * bitpix.bytes_per_pixel()`.
///
/// This is the single conversion path shared by full-frame reads (Phase 3),
/// the streaming row iterator (Phase 3), and — once it lands — region reads
/// (Phase 4).
pub(crate) fn decode<T: Pixel>(
    dst: &mut [T],
    src: &[u8],
    bitpix: BitPix,
    scaling: &Scaling,
) -> Result<(), FitsError> {
    let bpp = bitpix.bytes_per_pixel();
    let expected = dst.len().saturating_mul(bpp);
    if src.len() != expected {
        return Err(FitsError::BufferLenMismatch {
            expected,
            got: src.len(),
        });
    }

    match bitpix {
        BitPix::U8 => decode_int::<T, u8>(dst, src, scaling),
        BitPix::I16 => decode_int::<T, i16>(dst, src, scaling),
        BitPix::I32 => decode_int::<T, i32>(dst, src, scaling),
        BitPix::I64 => decode_int::<T, i64>(dst, src, scaling),
        BitPix::F32 => decode_float::<T, f32>(dst, src, scaling),
        BitPix::F64 => decode_float::<T, f64>(dst, src, scaling),
    }
    Ok(())
}

/// Integer `BITPIX`. Three cases, split so the common ones stay branch-free
/// per element (which is what lets the loop autovectorize — ADR 006 D2):
///
/// - no `BLANK`, integer offset applies (`BSCALE == 1`, integral `BZERO`):
///   one add per element. Covers the identity case (`BZERO == 0`) and every
///   `BZERO`-encoded unsigned convention (ADR 006 D6).
/// - no `BLANK`, real scaling: `BZERO + BSCALE * raw` in `f64`.
/// - `BLANK` present: per-element compare; a raw value equal to `BLANK`
///   becomes `NaN` for float targets and passes through unscaled for
///   integer targets (ADR 006 P3-T2).
fn decode_int<T: Pixel, R: RawSample>(dst: &mut [T], src: &[u8], s: &Scaling) {
    let chunks = src.chunks_exact(R::NBYTES);
    match (s.blank, s.int_offset()) {
        (None, Some(offset)) => {
            for (d, c) in dst.iter_mut().zip(chunks) {
                *d = T::from_i64(R::from_be(c).to_i64().wrapping_add(offset));
            }
        }
        (None, None) => {
            for (d, c) in dst.iter_mut().zip(chunks) {
                *d = T::from_phys(s.bzero + s.bscale * R::from_be(c).to_i64() as f64);
            }
        }
        (Some(blank), offset) => {
            for (d, c) in dst.iter_mut().zip(chunks) {
                let raw = R::from_be(c).to_i64();
                *d = if raw == blank {
                    if T::IS_FLOAT {
                        T::from_f64(f64::NAN)
                    } else {
                        T::from_i64(raw)
                    }
                } else if let Some(offset) = offset {
                    T::from_i64(raw.wrapping_add(offset))
                } else {
                    T::from_phys(s.bzero + s.bscale * raw as f64)
                };
            }
        }
    }
}

/// Floating `BITPIX` (`-32`, `-64`). `BLANK` does not apply — an undefined
/// pixel is already an IEEE `NaN` in-band (FITS Standard 4.0 §4.4.2.6).
fn decode_float<T: Pixel, R: RawSample>(dst: &mut [T], src: &[u8], s: &Scaling) {
    let chunks = src.chunks_exact(R::NBYTES);
    if s.is_identity() {
        for (d, c) in dst.iter_mut().zip(chunks) {
            *d = T::from_phys(R::from_be(c).to_f64());
        }
    } else {
        for (d, c) in dst.iter_mut().zip(chunks) {
            *d = T::from_phys(s.bzero + s.bscale * R::from_be(c).to_f64());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scaling(bscale: f64, bzero: f64, blank: Option<i64>) -> Scaling {
        Scaling {
            bscale,
            bzero,
            blank,
        }
    }

    #[test]
    fn u8_identity_roundtrips_every_byte() {
        let src: Vec<u8> = (0..=255u8).collect();
        let mut dst = [0u8; 256];
        decode(&mut dst, &src, BitPix::U8, &scaling(1.0, 0.0, None)).unwrap();
        assert!(dst.iter().enumerate().all(|(i, &v)| v as usize == i));
    }

    #[test]
    fn i16_big_endian_signed_decode() {
        let src = [
            0x00, 0x01, // 1
            0xFF, 0xFF, // -1
            0x80, 0x00, // i16::MIN
            0x7F, 0xFF, // i16::MAX
        ];
        let mut dst = [0i16; 4];
        decode(&mut dst, &src, BitPix::I16, &scaling(1.0, 0.0, None)).unwrap();
        assert_eq!(dst, [1, -1, i16::MIN, i16::MAX]);
    }

    #[test]
    fn bzero_32768_is_the_unsigned16_integer_fast_path() {
        // raw i16 spans its whole range; physical = raw + 32768 in [0, 65535].
        let src = [
            0x80, 0x00, // -32768 -> 0
            0x00, 0x00, // 0 -> 32768
            0x7F, 0xFF, // 32767 -> 65535
        ];
        let mut dst = [0u16; 3];
        decode(&mut dst, &src, BitPix::I16, &scaling(1.0, 32768.0, None)).unwrap();
        assert_eq!(dst, [0, 32768, 65535]);
    }

    #[test]
    fn real_bscale_bzero_applies_in_f64() {
        // physical = 10 + 0.5 * raw
        let src = [0x00, 0x00, 0x00, 0x0A]; // raw i32 = 10
        let mut dst = [0f64; 1];
        decode(&mut dst, &src, BitPix::I32, &scaling(0.5, 10.0, None)).unwrap();
        assert_eq!(dst[0], 15.0);
    }

    #[test]
    fn blank_becomes_nan_for_float_targets() {
        let src = [0x80, 0x00, 0x00, 0x2A]; // -32768 (BLANK), then 42
        let mut dst = [0f32; 2];
        decode(
            &mut dst,
            &src,
            BitPix::I16,
            &scaling(1.0, 0.0, Some(-32768)),
        )
        .unwrap();
        assert!(dst[0].is_nan());
        assert_eq!(dst[1], 42.0);
    }

    #[test]
    fn blank_passes_through_unscaled_for_integer_targets() {
        let src = [0x80, 0x00, 0x00, 0x2A]; // -32768 (BLANK), then 42
        let mut dst = [0i32; 2];
        decode(
            &mut dst,
            &src,
            BitPix::I16,
            &scaling(1.0, 1000.0, Some(-32768)),
        )
        .unwrap();
        assert_eq!(dst[0], -32768); // not offset by BZERO
        assert_eq!(dst[1], 1042); // 42 + 1000
    }

    #[test]
    fn f32_big_endian_decode() {
        let src = 1.5f32.to_be_bytes();
        let mut dst = [0f32; 1];
        decode(&mut dst, &src, BitPix::F32, &scaling(1.0, 0.0, None)).unwrap();
        assert_eq!(dst[0], 1.5);
    }

    #[test]
    fn length_mismatch_is_an_error() {
        let src = [0u8; 3];
        let mut dst = [0i16; 2]; // needs 4 bytes
        let err = decode(&mut dst, &src, BitPix::I16, &scaling(1.0, 0.0, None)).unwrap_err();
        assert!(matches!(
            err,
            FitsError::BufferLenMismatch {
                expected: 4,
                got: 3
            }
        ));
    }
}
