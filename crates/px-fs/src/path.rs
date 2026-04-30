use std::{
    path::{Path, PathBuf},
    sync::LazyLock,
};

use chrono::{NaiveDate, NaiveDateTime};
use regex::Regex;

/// The current working directory.
#[expect(clippy::print_stderr)]
pub static CWD: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::current_dir().unwrap_or_else(|_e| {
        eprintln!("Current directory does not exist");
        std::process::exit(1);
    })
});

pub trait Simplified {
    /// Simplify a [`Path`].
    ///
    /// On Windows, this will strip the `\\?\` prefix from paths. On other platforms, it's a no-op.
    fn simplified(&self) -> &Path;

    /// Render a [`Path`] for display.
    ///
    /// On Windows, this will strip the `\\?\` prefix from paths. On other platforms, it's
    /// equivalent to [`std::path::Display`].
    fn simplified_display(&self) -> impl std::fmt::Display;

    /// Canonicalize a path without a `\\?\` prefix on Windows.
    /// For a path that can't be canonicalized (e.g. on network drive or RAM drive on Windows),
    /// this will return the absolute path if it exists.
    fn simple_canonicalize(&self) -> std::io::Result<PathBuf>;

    /// Render a [`Path`] for user-facing display.
    ///
    /// Like [`simplified_display`], but relativizes the path against the current working directory.
    fn user_display(&self) -> impl std::fmt::Display;

    /// Render a [`Path`] for user-facing display, where the [`Path`] is relative to a base path.
    ///
    /// If the [`Path`] is not relative to the base path, will attempt to relativize the path
    /// against the current working directory.
    fn user_display_from(&self, base: impl AsRef<Path>) -> impl std::fmt::Display;
}

impl<T: AsRef<Path>> Simplified for T {
    fn simplified(&self) -> &Path {
        dunce::simplified(self.as_ref())
    }

    fn simplified_display(&self) -> impl std::fmt::Display {
        dunce::simplified(self.as_ref()).display()
    }

    fn simple_canonicalize(&self) -> std::io::Result<PathBuf> {
        dunce::canonicalize(self.as_ref())
    }

    fn user_display(&self) -> impl std::fmt::Display {
        let path = dunce::simplified(self.as_ref());

        // If current working directory is root, display the path as-is.
        if CWD.ancestors().nth(1).is_none() {
            return path.display();
        }

        // Attempt to strip the current working directory, then the canonicalized current working
        // directory, in case they differ.
        let path = path.strip_prefix(CWD.simplified()).unwrap_or(path);

        if path.as_os_str() == "" {
            // Avoid printing an empty string for the current directory
            return Path::new(".").display();
        }

        path.display()
    }

    fn user_display_from(&self, base: impl AsRef<Path>) -> impl std::fmt::Display {
        let path = dunce::simplified(self.as_ref());

        // If current working directory is root, display the path as-is.
        if CWD.ancestors().nth(1).is_none() {
            return path.display();
        }

        // Attempt to strip the base, then the current working directory, then the canonicalized
        // current working directory, in case they differ.
        let path = path
            .strip_prefix(base.as_ref())
            .unwrap_or_else(|_| path.strip_prefix(CWD.simplified()).unwrap_or(path));

        if path.as_os_str() == "" {
            // Avoid printing an empty string for the current directory
            return Path::new(".").display();
        }

        path.display()
    }
}

pub trait OptionPath {
    fn some_display(&self) -> Option<impl std::fmt::Display>;
    fn some_string(&self) -> Option<String>;
}

impl<T: AsRef<Path>> OptionPath for Option<T> {
    fn some_display(&self) -> Option<impl std::fmt::Display> {
        self.as_ref().map(|p| p.as_ref().display())
    }

    fn some_string(&self) -> Option<String> {
        self.some_display().map(|d| d.to_string())
    }
}

pub trait DatePath {
    /// Parse the file path looking for `YYYY-MM-DD` or `YYYYMMDD-hhmmss`
    ///
    fn with_date_time(&self) -> Option<NaiveDateTime>;
}

impl<T: AsRef<Path>> DatePath for T {
    fn with_date_time(&self) -> Option<NaiveDateTime> {
        // Look for a date + time in the path string (usually in file stem)
        let date_local_time = self.as_ref().to_str().and_then(|stem| {
            Regex::new(r"(\d{8}-\d{6})")
                .ok()?
                .captures(stem)
                .and_then(|caps| NaiveDateTime::parse_from_str(&caps[1], "%Y%m%d-%H%M%S").ok())
        });

        // Look for a date in the path string (usually in a folder)
        let date_local_date = self.as_ref().to_str().and_then(|parent| {
            Regex::new(r"(\d{4}-\d{2}-\d{2})")
                .ok()?
                .captures(parent)
                .and_then(|caps| NaiveDate::parse_from_str(&caps[1], "%Y-%m-%d").ok())
                .and_then(|d| d.and_hms_opt(0, 0, 0))
        });

        date_local_date.or(date_local_time)
    }
}
