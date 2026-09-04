//! `FitsReader`: lazy HDU navigation over a [`ByteSource`] (ADR 006 D7,
//! scoped to what Phase 2 delivers — a typed `Hdu` enum with dedicated
//! image/table/compressed-image variants arrives as those pieces land in
//! later phases; for now callers get [`DiscoveredHdu`], which already
//! carries a fully parsed [`Header`] and both HDUs' byte ranges).
//!
//! Discovery happens strictly on demand: opening a file or asking for HDU 0
//! never looks past the primary header's own blocks, and asking for HDU N
//! discovers HDUs `0..=N` (each header only, never data) and caches the
//! result so a repeated request doesn't re-scan.

use std::cell::{Cell, RefCell};
use std::path::Path;

use crate::error::FitsError;
use crate::hdu::{DiscoveredHdu, HduKind, discover_one};
use crate::image::ImageHdu;
use crate::source::{ByteSource, FileSource};

#[derive(Debug)]
pub struct FitsReader<S: ByteSource> {
    source: S,
    discovered: RefCell<Vec<DiscoveredHdu>>,
    /// Set once discovery has run off the end of the source, so repeated
    /// out-of-range requests don't re-scan to confirm it again.
    exhausted: Cell<bool>,
}

impl FitsReader<FileSource> {
    /// Opens `path` and reads the primary header. Reads only the primary
    /// header's blocks — no other HDU is touched until requested.
    pub fn open(path: impl AsRef<Path>) -> Result<Self, FitsError> {
        let source = FileSource::open(path)?;
        Self::from_source(source)
    }
}

impl<S: ByteSource> FitsReader<S> {
    /// Wraps an already-open [`ByteSource`]. Reads only the primary header.
    pub fn from_source(source: S) -> Result<Self, FitsError> {
        let reader = Self {
            source,
            discovered: RefCell::new(Vec::new()),
            exhausted: Cell::new(false),
        };
        // Eagerly discovering the primary HDU (and only the primary HDU)
        // matches `FitsFile::new`'s prior behaviour of failing fast if the
        // file has no valid primary header, while still satisfying the
        // laziness requirement for every HDU after it.
        reader.discover_up_to(0)?;
        Ok(reader)
    }

    fn discover_up_to(&self, index: usize) -> Result<(), FitsError> {
        loop {
            if self.discovered.borrow().len() > index {
                return Ok(());
            }
            if self.exhausted.get() {
                return Err(FitsError::HduIndexOutOfRange(index));
            }

            let have = self.discovered.borrow().len();
            let next_offset = match self.discovered.borrow().last() {
                Some(last) => last.next_header_offset(),
                None => 0,
            };
            if next_offset >= self.source.len() {
                self.exhausted.set(true);
                return Err(FitsError::HduIndexOutOfRange(index));
            }

            let hdu = discover_one(&self.source, next_offset, have == 0)?;
            self.discovered.borrow_mut().push(hdu);
        }
    }

    /// The primary HDU (index 0). Already discovered by [`from_source`] /
    /// [`open`], so this never re-reads.
    pub fn primary(&self) -> Result<DiscoveredHdu, FitsError> {
        self.hdu(0)
    }

    /// The HDU at `index` (0 = primary). Discovers `0..=index` if not
    /// already cached; never reads any HDU's data.
    pub fn hdu(&self, index: usize) -> Result<DiscoveredHdu, FitsError> {
        self.discover_up_to(index)?;
        Ok(self.discovered.borrow()[index].clone())
    }

    /// Iterates all HDUs from index 0, discovering lazily as the iterator
    /// advances.
    pub fn hdus(&self) -> HduIter<'_, S> {
        HduIter {
            reader: self,
            index: 0,
            done: false,
        }
    }

    /// The total HDU count. Forces discovery of every HDU in the file.
    pub fn hdu_count(&self) -> Result<usize, FitsError> {
        let mut index = self.discovered.borrow().len();
        loop {
            match self.discover_up_to(index) {
                Ok(()) => index += 1,
                Err(FitsError::HduIndexOutOfRange(_)) => break,
                Err(e) => return Err(e),
            }
        }
        Ok(self.discovered.borrow().len())
    }

    pub fn source(&self) -> &S {
        &self.source
    }

    /// Typed image access for the HDU at `index`. Accepts the primary HDU and
    /// `IMAGE` extensions; a table or unknown extension type is
    /// [`FitsError::NotAnImage`]. Typed table access is Phase 6, compressed
    /// images Phase 7.
    pub fn image(&self, index: usize) -> Result<ImageHdu<'_, S>, FitsError> {
        let hdu = self.hdu(index)?;
        match &hdu.kind {
            HduKind::Primary | HduKind::Image => ImageHdu::from_discovered(self.source(), hdu),
            other => Err(FitsError::NotAnImage {
                index,
                kind: format!("{other:?}"),
            }),
        }
    }

    /// Typed image access for the primary HDU (index 0).
    pub fn primary_image(&self) -> Result<ImageHdu<'_, S>, FitsError> {
        self.image(0)
    }
}

/// Lazily discovers and yields each [`DiscoveredHdu`] in file order,
/// starting from index 0. Stops (returns `None`) once discovery runs off
/// the end of the source; a genuine I/O or parse error is yielded once and
/// then also ends iteration.
pub struct HduIter<'a, S: ByteSource> {
    reader: &'a FitsReader<S>,
    index: usize,
    done: bool,
}

impl<S: ByteSource> Iterator for HduIter<'_, S> {
    type Item = Result<DiscoveredHdu, FitsError>;

    fn next(&mut self) -> Option<Self::Item> {
        if self.done {
            return None;
        }
        match self.reader.hdu(self.index) {
            Ok(hdu) => {
                self.index += 1;
                Some(Ok(hdu))
            }
            Err(FitsError::HduIndexOutOfRange(_)) => {
                self.done = true;
                None
            }
            Err(e) => {
                self.done = true;
                Some(Err(e))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::card::{Card, Value};
    use crate::hdu::HduKind;
    use crate::source::SliceSource;

    fn image_header(naxis: &[i64], extra: &[Card]) -> Vec<Card> {
        let mut cards = vec![
            Card::new("SIMPLE", Value::Logical(true), None),
            Card::new("BITPIX", Value::Integer(16), None),
            Card::new("NAXIS", Value::Integer(naxis.len() as i64), None),
        ];
        for (i, n) in naxis.iter().enumerate() {
            cards.push(Card::new(
                format!("NAXIS{}", i + 1),
                Value::Integer(*n),
                None,
            ));
        }
        cards.extend_from_slice(extra);
        cards
    }

    fn extension_header(naxis: &[i64]) -> Vec<Card> {
        let mut cards = vec![
            Card::new("XTENSION", Value::String("IMAGE".to_string()), None),
            Card::new("BITPIX", Value::Integer(8), None),
            Card::new("NAXIS", Value::Integer(naxis.len() as i64), None),
        ];
        for (i, n) in naxis.iter().enumerate() {
            cards.push(Card::new(
                format!("NAXIS{}", i + 1),
                Value::Integer(*n),
                None,
            ));
        }
        cards.push(Card::new("PCOUNT", Value::Integer(0), None));
        cards.push(Card::new("GCOUNT", Value::Integer(1), None));
        cards
    }

    fn card_block(cards: &[Card]) -> Vec<u8> {
        let mut bytes = Vec::new();
        for c in cards {
            bytes.extend_from_slice(&c.to_bytes());
        }
        bytes.extend_from_slice(
            &Card::new("END", Value::Commentary(String::new()), None).to_bytes(),
        );
        let rem = bytes.len() % crate::block::BLOCK_SIZE;
        if rem != 0 {
            bytes.resize(bytes.len() + (crate::block::BLOCK_SIZE - rem), b' ');
        }
        bytes
    }

    /// Builds a single-HDU file: header + `data_blocks` padded data blocks.
    fn single_hdu_file(naxis: &[i64], data_blocks: usize) -> Vec<u8> {
        let mut bytes = card_block(&image_header(naxis, &[]));
        bytes.resize(bytes.len() + data_blocks * crate::block::BLOCK_SIZE, 0);
        bytes
    }

    /// Builds a primary (NAXIS=0, no data) + one IMAGE extension file.
    fn two_hdu_file(ext_naxis: &[i64]) -> Vec<u8> {
        let mut bytes = card_block(&image_header(&[], &[]));
        let ext_header = card_block(&extension_header(ext_naxis));
        let ext_data_len: i64 = ext_naxis.iter().product();
        bytes.extend_from_slice(&ext_header);
        let padded = crate::block::padded_len(ext_data_len as u64) as usize;
        bytes.resize(bytes.len() + padded, 0);
        bytes
    }

    #[test]
    fn open_reads_only_primary_header() {
        let bytes = two_hdu_file(&[12, 9]);
        let source = SliceSource::new(bytes);
        let reader = FitsReader::from_source(source).unwrap();

        let primary = reader.primary().unwrap();
        assert_eq!(primary.kind, HduKind::Primary);
        assert_eq!(primary.data_len, 0);
    }

    #[test]
    fn hdu_discovers_extension_lazily() {
        let bytes = two_hdu_file(&[12, 9]);
        let source = SliceSource::new(bytes);
        let reader = FitsReader::from_source(source).unwrap();

        let ext = reader.hdu(1).unwrap();
        assert_eq!(ext.kind, HduKind::Image);
        assert_eq!(ext.data_len, 12 * 9); // BITPIX=8 -> 1 byte/pixel
    }

    #[test]
    fn hdu_out_of_range_errors() {
        let bytes = single_hdu_file(&[4, 4], 1);
        let source = SliceSource::new(bytes);
        let reader = FitsReader::from_source(source).unwrap();

        let err = reader.hdu(1).unwrap_err();
        assert!(matches!(err, FitsError::HduIndexOutOfRange(1)));
    }

    #[test]
    fn hdu_count_matches_actual_hdu_count() {
        let bytes = two_hdu_file(&[5, 5]);
        let source = SliceSource::new(bytes);
        let reader = FitsReader::from_source(source).unwrap();

        assert_eq!(reader.hdu_count().unwrap(), 2);
    }

    #[test]
    fn hdus_iterator_yields_all_hdus_in_order() {
        let bytes = two_hdu_file(&[3, 3]);
        let source = SliceSource::new(bytes);
        let reader = FitsReader::from_source(source).unwrap();

        let kinds: Vec<HduKind> = reader.hdus().map(|h| h.unwrap().kind).collect();
        assert_eq!(kinds, vec![HduKind::Primary, HduKind::Image]);
    }

    #[test]
    fn repeated_hdu_access_does_not_rediscover() {
        let bytes = two_hdu_file(&[3, 3]);
        let source = SliceSource::new(bytes);
        let reader = FitsReader::from_source(source).unwrap();

        let first = reader.hdu(1).unwrap();
        let second = reader.hdu(1).unwrap();
        assert_eq!(first.header_offset, second.header_offset);
        assert_eq!(reader.discovered.borrow().len(), 2);
    }

    #[test]
    fn open_missing_file_errors() {
        let err = FitsReader::open("/nonexistent/path/does/not/exist.fits").unwrap_err();
        assert!(matches!(err, FitsError::Io(_)));
    }
}
