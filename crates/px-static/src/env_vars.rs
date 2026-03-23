/// Declares all environment variable used throughout `px` and its crates.
pub struct EnvVars;

impl EnvVars {
    /// The directory containing the `Cargo.toml` manifest for a package.
    pub const CARGO_MANIFEST_DIR: &'static str = "CARGO_MANIFEST_DIR";

    /// Equivalent to the `--token` argument for self update. A GitHub token for authentication.
    pub const PX_GITHUB_TOKEN: &'static str = "PX_GITHUB_TOKEN";

    /// Used to set `RUST_HOST_TARGET` at build time via `build.rs`.
    pub const TARGET: &'static str = "TARGET";

    /// Used to set the uv commit hash at build time via `build.rs`.
    pub const PX_COMMIT_HASH: &'static str = "PX_COMMIT_HASH";

    /// Used to set the uv commit short hash at build time via `build.rs`.
    pub const PX_COMMIT_SHORT_HASH: &'static str = "PX_COMMIT_SHORT_HASH";

    /// Used to set the uv commit date at build time via `build.rs`.
    pub const PX_COMMIT_DATE: &'static str = "PX_COMMIT_DATE";

    /// Used to set the uv tag at build time via `build.rs`.
    pub const PX_LAST_TAG: &'static str = "PX_LAST_TAG";

    /// Used to set the uv tag distance from head at build time via `build.rs`.
    pub const PX_LAST_TAG_DISTANCE: &'static str = "PX_LAST_TAG_DISTANCE";
}
