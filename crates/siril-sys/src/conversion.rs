use std::{
    io,
    path::{Path, PathBuf},
};

use regex::Regex;

/// A single entry in a Siril conversion file, mapping an original filename to its converted name.
#[derive(Debug)]
pub struct ConversionEntry {
    pub original_file: PathBuf,
    pub converted_file: PathBuf,
}

/// A conversion file produced by Siril's `convert` command.
///
/// The file contains lines of the form `'original' -> 'converted'`, which this type parses
/// into a list of [`ConversionEntry`] values.
#[derive(Debug)]
pub struct ConversionFile {
    pub entries: Vec<ConversionEntry>,
    pub file: PathBuf,
}

impl ConversionFile {
    pub fn new(file: PathBuf) -> io::Result<Self> {
        let mut this = Self {
            entries: Vec::new(),
            file,
        };
        this.read()?;
        Ok(this)
    }

    fn read(&mut self) -> io::Result<()> {
        if !self.file.exists() {
            return Ok(());
        }

        let raw = std::fs::read_to_string(&self.file)?;
        let re = Regex::new(r"'(.*?)'.*?'(.*?)'").expect("hardcoded regex is valid");
        for caps in re.captures_iter(&raw) {
            self.entries.push(ConversionEntry {
                original_file: PathBuf::from(&caps[1]),
                converted_file: PathBuf::from(&caps[2]),
            });
        }

        Ok(())
    }

    /// Move converted files into `output_folder`, renaming each back to its original filename
    /// with `prefix` prepended.
    ///
    /// For each entry, the file at `<conversion_file_parent>/<prefix><converted_name>` is
    /// moved to `<output_folder>/<prefix><original_name>`.
    pub fn move_converted_files(&self, output_folder: &Path, prefix: &str) -> io::Result<()> {
        let parent = self.file.parent().unwrap_or(Path::new("."));
        for entry in &self.entries {
            let src = parent.join(format!(
                "{}{}",
                prefix,
                entry
                    .converted_file
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ));
            let dst = output_folder.join(format!(
                "{}{}",
                prefix,
                entry
                    .original_file
                    .file_name()
                    .unwrap_or_default()
                    .to_string_lossy()
            ));
            std::fs::rename(src, dst)?;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use tempfile::TempDir;

    use super::*;

    const SAMPLE: &str = "\
'/Users/kyle/Pictures/Astro/PX_Radian75/LIGHT/NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ultra/Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-221643_-10.0C_0001.fit' -> '/var/folders/fq/98jp68ps1xbfgwsmm9_fm17m0000gn/T/photonyx-tv8cyR/light_00001.fit'
'/Users/kyle/Pictures/Astro/PX_Radian75/LIGHT/NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ultra/Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-222758_-10.0C_0003.fit' -> '/var/folders/fq/98jp68ps1xbfgwsmm9_fm17m0000gn/T/photonyx-tv8cyR/light_00002.fit'
'/Users/kyle/Pictures/Astro/PX_Radian75/LIGHT/NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ultra/Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-223328_-9.4C_0004.fit' -> '/var/folders/fq/98jp68ps1xbfgwsmm9_fm17m0000gn/T/photonyx-tv8cyR/light_00003.fit'
'/Users/kyle/Pictures/Astro/PX_Radian75/LIGHT/NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ultra/Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-223854_-10.0C_0005.fit' -> '/var/folders/fq/98jp68ps1xbfgwsmm9_fm17m0000gn/T/photonyx-tv8cyR/light_00004.fit'
'/Users/kyle/Pictures/Astro/PX_Radian75/LIGHT/NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ultra/Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-224542_-9.4C_0006.fit' -> '/var/folders/fq/98jp68ps1xbfgwsmm9_fm17m0000gn/T/photonyx-tv8cyR/light_00005.fit'
'/Users/kyle/Pictures/Astro/PX_Radian75/LIGHT/NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ultra/Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-225052_-10.0C_0007.fit' -> '/var/folders/fq/98jp68ps1xbfgwsmm9_fm17m0000gn/T/photonyx-tv8cyR/light_00006.fit'
'/Users/kyle/Pictures/Astro/PX_Radian75/LIGHT/NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ultra/Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-225823_-10.0C_0008.fit' -> '/var/folders/fq/98jp68ps1xbfgwsmm9_fm17m0000gn/T/photonyx-tv8cyR/light_00007.fit'
'/Users/kyle/Pictures/Astro/PX_Radian75/LIGHT/NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ultra/Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-230358_-10.0C_0009.fit' -> '/var/folders/fq/98jp68ps1xbfgwsmm9_fm17m0000gn/T/photonyx-tv8cyR/light_00008.fit'
'/Users/kyle/Pictures/Astro/PX_Radian75/LIGHT/NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ultra/Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-230925_-10.0C_0010.fit' -> '/var/folders/fq/98jp68ps1xbfgwsmm9_fm17m0000gn/T/photonyx-tv8cyR/light_00009.fit'
'/Users/kyle/Pictures/Astro/PX_Radian75/LIGHT/NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ultra/Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-231507_-10.0C_0011.fit' -> '/var/folders/fq/98jp68ps1xbfgwsmm9_fm17m0000gn/T/photonyx-tv8cyR/light_00010.fit'
'/Users/kyle/Pictures/Astro/PX_Radian75/LIGHT/NGC_7000_NA_Nebula/2025-06-25/RAW_300_Ultra/Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-232040_-10.0C_0012.fit' -> '/var/folders/fq/98jp68ps1xbfgwsmm9_fm17m0000gn/T/photonyx-tv8cyR/light_00011.fit'";

    fn write_conversion_file(dir: &TempDir) -> PathBuf {
        let path = dir.path().join("conversion.txt");
        fs::write(&path, SAMPLE).unwrap();
        path
    }

    #[test]
    fn parses_all_entries() {
        let dir = TempDir::new().unwrap();
        let cf = ConversionFile::new(write_conversion_file(&dir)).unwrap();
        assert_eq!(cf.entries.len(), 11);
    }

    #[test]
    fn parses_first_entry_correctly() {
        let dir = TempDir::new().unwrap();
        let cf = ConversionFile::new(write_conversion_file(&dir)).unwrap();
        let first = &cf.entries[0];
        assert_eq!(
            first.original_file.file_name().unwrap(),
            "Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-221643_-10.0C_0001.fit"
        );
        assert_eq!(first.converted_file.file_name().unwrap(), "light_00001.fit");
    }

    #[test]
    fn parses_last_entry_correctly() {
        let dir = TempDir::new().unwrap();
        let cf = ConversionFile::new(write_conversion_file(&dir)).unwrap();
        let last = cf.entries.last().unwrap();
        assert_eq!(
            last.original_file.file_name().unwrap(),
            "Light_NGC 7000_300.0s_Bin1_071MC_gain90_20250625-232040_-10.0C_0012.fit"
        );
        assert_eq!(last.converted_file.file_name().unwrap(), "light_00011.fit");
    }

    #[test]
    fn missing_file_yields_empty_entries() {
        let cf = ConversionFile::new(PathBuf::from("/nonexistent/conversion.txt")).unwrap();
        assert!(cf.entries.is_empty());
    }

    #[test]
    fn move_converted_files_renames_correctly() {
        let dir = TempDir::new().unwrap();
        let conv_file = write_conversion_file(&dir);
        let cf = ConversionFile::new(conv_file).unwrap();

        // Create stub converted files in the same dir as the conversion file
        let prefix = "light_";
        for entry in &cf.entries {
            let name = entry.converted_file.file_name().unwrap();
            fs::write(
                dir.path()
                    .join(format!("{prefix}{}", name.to_string_lossy())),
                b"",
            )
            .unwrap();
        }

        let output_dir = TempDir::new().unwrap();
        cf.move_converted_files(output_dir.path(), prefix).unwrap();

        // Each original filename (with prefix) should now exist in the output folder
        for entry in &cf.entries {
            let expected = output_dir.path().join(format!(
                "{prefix}{}",
                entry.original_file.file_name().unwrap().to_string_lossy()
            ));
            assert!(expected.exists(), "expected {expected:?} to exist");
        }
    }
}
