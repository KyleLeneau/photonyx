//! FITS block arithmetic: every header and data unit is a whole multiple of
//! `BLOCK_SIZE` bytes (FITS Standard 4.0 §3.3.2 "FITS blocks"). These are
//! pure functions so Phase 2's HDU discovery (locating the next HDU from the
//! current one's `BITPIX`/`NAXISn` without reading its data) can be unit
//! tested independently of any I/O.

/// The fixed FITS block size in bytes, per the standard.
pub const BLOCK_SIZE: usize = 2880;

/// The size, in bytes, of one 80-character card image.
pub const CARD_SIZE: usize = 80;

/// Cards per block (`BLOCK_SIZE / CARD_SIZE`).
pub const CARDS_PER_BLOCK: usize = BLOCK_SIZE / CARD_SIZE;

/// Number of whole blocks required to hold `len` bytes, rounding up. FITS
/// data and header units are always padded to a block boundary, so this is
/// "how many blocks does this unit occupy on disk", not merely `ceil` of a
/// byte count.
pub fn block_count(len: u64) -> u64 {
    len.div_ceil(BLOCK_SIZE as u64)
}

/// The padded size, in bytes, of a unit of length `len` — `len` rounded up
/// to the next multiple of [`BLOCK_SIZE`]. Zero-length units still occupy
/// zero bytes (an empty data unit, e.g. `NAXIS = 0`, is not padded to a
/// block — see `padded_len_data_unit` for the case that must never be zero).
pub fn padded_len(len: u64) -> u64 {
    block_count(len) * BLOCK_SIZE as u64
}

/// Number of trailing padding bytes needed to bring `len` up to a block
/// boundary. Zero when `len` is already a multiple of [`BLOCK_SIZE`],
/// including when `len == 0`.
pub fn padding_for(len: u64) -> u64 {
    padded_len(len) - len
}

/// True when `len` is already exactly a whole number of blocks (including
/// zero).
pub fn is_block_aligned(len: u64) -> bool {
    len.is_multiple_of(BLOCK_SIZE as u64)
}

/// Converts a byte offset within a source to the (block index, byte-within-
/// block) pair. Useful for reasoning about which physical block an offset
/// falls in, e.g. when deciding whether an in-place header rewrite (Phase 5
/// `update_header`) fits without moving subsequent data.
pub fn offset_to_block(offset: u64) -> (u64, u64) {
    (offset / BLOCK_SIZE as u64, offset % BLOCK_SIZE as u64)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_count_zero_is_zero() {
        assert_eq!(block_count(0), 0);
    }

    #[test]
    fn block_count_exact_multiple() {
        assert_eq!(block_count(BLOCK_SIZE as u64), 1);
        assert_eq!(block_count(BLOCK_SIZE as u64 * 3), 3);
    }

    #[test]
    fn block_count_rounds_up() {
        assert_eq!(block_count(1), 1);
        assert_eq!(block_count(BLOCK_SIZE as u64 + 1), 2);
        assert_eq!(block_count(BLOCK_SIZE as u64 - 1), 1);
    }

    #[test]
    fn padded_len_matches_block_count_times_block_size() {
        for len in [0u64, 1, 2879, 2880, 2881, 5760, 5761] {
            assert_eq!(padded_len(len), block_count(len) * BLOCK_SIZE as u64);
        }
    }

    #[test]
    fn padding_for_zero_length_is_zero() {
        assert_eq!(padding_for(0), 0);
    }

    #[test]
    fn padding_for_exact_multiple_is_zero() {
        assert_eq!(padding_for(BLOCK_SIZE as u64), 0);
        assert_eq!(padding_for(BLOCK_SIZE as u64 * 4), 0);
    }

    #[test]
    fn padding_for_partial_block() {
        assert_eq!(padding_for(1), BLOCK_SIZE as u64 - 1);
        assert_eq!(padding_for(BLOCK_SIZE as u64 + 1), BLOCK_SIZE as u64 - 1);
    }

    #[test]
    fn is_block_aligned_cases() {
        assert!(is_block_aligned(0));
        assert!(is_block_aligned(BLOCK_SIZE as u64));
        assert!(is_block_aligned(BLOCK_SIZE as u64 * 2));
        assert!(!is_block_aligned(1));
        assert!(!is_block_aligned(BLOCK_SIZE as u64 - 1));
        assert!(!is_block_aligned(BLOCK_SIZE as u64 + 1));
    }

    #[test]
    fn offset_to_block_start_of_block() {
        assert_eq!(offset_to_block(0), (0, 0));
        assert_eq!(offset_to_block(BLOCK_SIZE as u64), (1, 0));
        assert_eq!(offset_to_block(BLOCK_SIZE as u64 * 2), (2, 0));
    }

    #[test]
    fn offset_to_block_mid_block() {
        assert_eq!(offset_to_block(1), (0, 1));
        assert_eq!(offset_to_block(BLOCK_SIZE as u64 + 5), (1, 5));
        assert_eq!(
            offset_to_block(BLOCK_SIZE as u64 - 1),
            (0, BLOCK_SIZE as u64 - 1)
        );
    }

    #[test]
    fn cards_per_block_is_36() {
        assert_eq!(CARDS_PER_BLOCK, 36);
        assert_eq!(CARDS_PER_BLOCK * CARD_SIZE, BLOCK_SIZE);
    }
}
