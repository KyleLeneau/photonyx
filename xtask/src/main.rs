//! See <https://github.com/matklad/cargo-xtask/>.
//!
//! This binary defines various auxiliary build commands, which are not
//! expressible with just `cargo`.
//!
//! This binary is integrated into the `cargo` command line by using an alias in
//! `.cargo/config`.

use std::{env, path::PathBuf};
use xshell::{Shell, cmd};

fn main() -> anyhow::Result<()> {
    println!("Hello xtask helper from: {:?}", project_root());

    let sh = &Shell::new()?;
    sh.change_dir(project_root());

    cmd!(&sh, "cargo --version").run()?;

    Ok(())
}

/// Returns the path to the root directory of `photonyx` project.
fn project_root() -> PathBuf {
    let dir =
        env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    PathBuf::from(dir).parent().unwrap().to_owned()
}
