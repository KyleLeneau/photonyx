/// Declares all environment variable used throughout `px` and its crates.
pub struct EnvVars;

impl EnvVars {
    /// The directory containing the `Cargo.toml` manifest for a package.
    pub const CARGO_MANIFEST_DIR: &'static str = "CARGO_MANIFEST_DIR";

    /// Equivalent to the `--token` argument for self update. A GitHub token for authentication.
    pub const PX_GITHUB_TOKEN: &'static str = "PX_GITHUB_TOKEN";

    /// Used to set `RUST_HOST_TARGET` at build time via `build.rs`.
    pub const TARGET: &'static str = "TARGET";

    /// Used to set the px commit hash at build time via `build.rs`.
    pub const PX_COMMIT_HASH: &'static str = "PX_COMMIT_HASH";

    /// Used to set the px commit short hash at build time via `build.rs`.
    pub const PX_COMMIT_SHORT_HASH: &'static str = "PX_COMMIT_SHORT_HASH";

    /// Used to set the px commit date at build time via `build.rs`.
    pub const PX_COMMIT_DATE: &'static str = "PX_COMMIT_DATE";

    /// Used to set the px tag at build time via `build.rs`.
    pub const PX_LAST_TAG: &'static str = "PX_LAST_TAG";

    /// Used to set the px tag distance from head at build time via `build.rs`.
    pub const PX_LAST_TAG_DISTANCE: &'static str = "PX_LAST_TAG_DISTANCE";

    /// Used to set the default fit extension for px (default: `fit`)
    pub const PX_DEFAULT_FIT_EXT: &'static str = "PX_DEFAULT_FIT_EXT";

    /// Used to set the default output format of the cli (pretty or json)
    pub const PX_DEFAULT_OUTPUT_FORMAT: &'static str = "PX_DEFAULT_OUTPUT_FORMAT";

    /// Equivalent to the `--profile` command-line argument.
    pub const PX_PROFILE: &'static str = "PX_PROFILE";
}
