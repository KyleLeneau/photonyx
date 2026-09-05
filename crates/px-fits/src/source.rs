//! Byte sources: the read abstraction the native FITS reader is built on
//! (ADR 006 D1). Positioned reads (`FileSource`) are the *only* backend —
//! no `unsafe`, no shared cursor, safe to call concurrently from multiple
//! `rayon` workers over one open handle. `SliceSource` supports in-memory
//! use (tests, and any future embedding of already-loaded bytes).
//!
//! A memory-mapped backend was built and benchmarked in Phase 8 (P8-T1..T7)
//! and then removed: `rayon`-parallelized positioned reads beat it on
//! full-frame reads (up to ~3.9x) and header scans, and it won by only
//! ~10% on a warm-cache region read — not enough to justify the `unsafe`,
//! the `memmap2` dependency, and the file-truncation footgun. See the
//! backend report in `README.md`.

use std::fs::File;
use std::io;
use std::path::Path;

/// A random-access source of bytes. Implementations must be safe to read
/// from concurrently through a shared reference — reads never mutate shared
/// cursor state.
pub trait ByteSource: Send + Sync {
    /// Total length of the source, in bytes.
    fn len(&self) -> u64;

    /// True when the source is empty.
    fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Fills `buf` entirely with bytes starting at `offset`. Errors with
    /// `UnexpectedEof` if fewer than `buf.len()` bytes are available from
    /// `offset` to the end of the source.
    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()>;

    /// A zero-copy borrow of the whole source, when the backend can provide
    /// one without an intermediate read. `FileSource` returns `None`;
    /// `SliceSource` returns `Some`.
    fn as_slice(&self) -> Option<&[u8]> {
        None
    }
}

/// The default backend: positioned reads against an open file handle.
/// `read_at`/`seek_read` take `&self` (no seek-then-read races), so a single
/// `FileSource` can be shared across threads without locking.
#[derive(Debug)]
pub struct FileSource {
    file: File,
    len: u64,
}

impl FileSource {
    pub fn open(path: impl AsRef<Path>) -> io::Result<Self> {
        let file = File::open(path)?;
        let len = file.metadata()?.len();
        Ok(Self { file, len })
    }

    pub fn from_file(file: File) -> io::Result<Self> {
        let len = file.metadata()?.len();
        Ok(Self { file, len })
    }
}

impl ByteSource for FileSource {
    fn len(&self) -> u64 {
        self.len
    }

    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        positioned::read_exact_at(&self.file, buf, offset)
    }
}

/// An in-memory source, e.g. for tests or bytes already loaded by a caller.
#[derive(Debug)]
pub struct SliceSource(Vec<u8>);

impl SliceSource {
    pub fn new(bytes: Vec<u8>) -> Self {
        Self(bytes)
    }
}

impl ByteSource for SliceSource {
    fn len(&self) -> u64 {
        self.0.len() as u64
    }

    fn read_exact_at(&self, buf: &mut [u8], offset: u64) -> io::Result<()> {
        let start = usize::try_from(offset)
            .map_err(|_| io::Error::new(io::ErrorKind::UnexpectedEof, "offset overflows usize"))?;
        let end = start
            .checked_add(buf.len())
            .ok_or_else(|| io::Error::new(io::ErrorKind::UnexpectedEof, "read range overflows"))?;
        let slice = self.0.get(start..end).ok_or_else(|| {
            io::Error::new(io::ErrorKind::UnexpectedEof, "read past end of slice")
        })?;
        buf.copy_from_slice(slice);
        Ok(())
    }

    fn as_slice(&self) -> Option<&[u8]> {
        Some(&self.0)
    }
}

/// Cross-platform positioned-read shim. Neither `read_at` (Unix) nor
/// `seek_read` (Windows) is guaranteed to fill the buffer in one call, so
/// both loop, advancing `offset` and the destination slice on partial reads.
#[cfg(unix)]
mod positioned {
    use std::fs::File;
    use std::io;
    use std::os::unix::fs::FileExt;

    pub fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
        while !buf.is_empty() {
            match file.read_at(buf, offset) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "read past end of file",
                    ));
                }
                Ok(n) => {
                    buf = &mut buf[n..];
                    offset += n as u64;
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

#[cfg(windows)]
mod positioned {
    use std::fs::File;
    use std::io;
    use std::os::windows::fs::FileExt;

    /// `seek_read` moves the file's internal cursor as a side effect, but
    /// since every call here supplies an explicit offset, concurrent callers
    /// each converge on the offset they asked for rather than relying on
    /// cursor state — correctness holds even though the side effect exists.
    pub fn read_exact_at(file: &File, mut buf: &mut [u8], mut offset: u64) -> io::Result<()> {
        while !buf.is_empty() {
            match file.seek_read(buf, offset) {
                Ok(0) => {
                    return Err(io::Error::new(
                        io::ErrorKind::UnexpectedEof,
                        "read past end of file",
                    ));
                }
                Ok(n) => {
                    buf = &mut buf[n..];
                    offset += n as u64;
                }
                Err(ref e) if e.kind() == io::ErrorKind::Interrupted => continue,
                Err(e) => return Err(e),
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn temp_file(contents: &[u8]) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().expect("create temp dir");
        let path = dir.path().join("source_test.bin");
        let mut f = File::create(&path).expect("create temp file");
        f.write_all(contents).expect("write temp file");
        (dir, path)
    }

    #[test]
    fn file_source_reads_at_offset() {
        let (_dir, path) = temp_file(b"0123456789");
        let src = FileSource::open(&path).unwrap();
        assert_eq!(src.len(), 10);

        let mut buf = [0u8; 4];
        src.read_exact_at(&mut buf, 3).unwrap();
        assert_eq!(&buf, b"3456");
    }

    #[test]
    fn file_source_reads_from_zero() {
        let (_dir, path) = temp_file(b"abcdef");
        let src = FileSource::open(&path).unwrap();
        let mut buf = [0u8; 3];
        src.read_exact_at(&mut buf, 0).unwrap();
        assert_eq!(&buf, b"abc");
    }

    #[test]
    fn file_source_reads_exact_final_bytes() {
        let (_dir, path) = temp_file(b"abcdef");
        let src = FileSource::open(&path).unwrap();
        let mut buf = [0u8; 3];
        src.read_exact_at(&mut buf, 3).unwrap();
        assert_eq!(&buf, b"def");
    }

    #[test]
    fn file_source_short_read_past_eof_errors() {
        let (_dir, path) = temp_file(b"abcdef");
        let src = FileSource::open(&path).unwrap();
        let mut buf = [0u8; 4];
        let err = src.read_exact_at(&mut buf, 4).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn file_source_offset_at_eof_with_zero_len_read_is_ok() {
        let (_dir, path) = temp_file(b"abcdef");
        let src = FileSource::open(&path).unwrap();
        let mut buf: [u8; 0] = [];
        src.read_exact_at(&mut buf, 6).unwrap();
    }

    #[test]
    fn file_source_offset_beyond_eof_errors() {
        let (_dir, path) = temp_file(b"abcdef");
        let src = FileSource::open(&path).unwrap();
        let mut buf = [0u8; 1];
        let err = src.read_exact_at(&mut buf, 100).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn file_source_shares_across_threads_without_locking() {
        let (_dir, path) = temp_file(&(0u8..=255).collect::<Vec<u8>>());
        let src = std::sync::Arc::new(FileSource::open(&path).unwrap());

        std::thread::scope(|scope| {
            for i in 0..16u64 {
                let src = std::sync::Arc::clone(&src);
                scope.spawn(move || {
                    let mut buf = [0u8; 8];
                    src.read_exact_at(&mut buf, i * 8).unwrap();
                    for (j, b) in buf.iter().enumerate() {
                        assert_eq!(*b, (i * 8 + j as u64) as u8);
                    }
                });
            }
        });
    }

    #[test]
    fn slice_source_reads_at_offset_and_exposes_as_slice() {
        let src = SliceSource::new(b"hello world".to_vec());
        assert_eq!(src.len(), 11);
        assert_eq!(src.as_slice(), Some(b"hello world".as_slice()));

        let mut buf = [0u8; 5];
        src.read_exact_at(&mut buf, 6).unwrap();
        assert_eq!(&buf, b"world");
    }

    #[test]
    fn slice_source_short_read_past_end_errors() {
        let src = SliceSource::new(b"abc".to_vec());
        let mut buf = [0u8; 4];
        let err = src.read_exact_at(&mut buf, 0).unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::UnexpectedEof);
    }

    #[test]
    fn slice_source_zero_length_read_at_exact_end_is_ok() {
        let src = SliceSource::new(b"abc".to_vec());
        let mut buf: [u8; 0] = [];
        src.read_exact_at(&mut buf, 3).unwrap();
    }
}
