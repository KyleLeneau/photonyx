use std::io::Result;
use std::path::Path;

pub trait Glob {
    /// Counts the file in the path that match the file extension
    ///
    fn count_by_ext(&self, ext: String) -> Result<usize>;
}

impl<T: AsRef<Path>> Glob for T {
    fn count_by_ext(&self, ext: String) -> Result<usize> {
        let count = std::fs::read_dir(self)?
            .filter_map(|e| e.ok())
            .filter(|e| {
                e.path()
                    .extension()
                    .is_some_and(|x| x.eq_ignore_ascii_case(ext.as_str()))
            })
            .count();
        Ok(count)
    }
}
