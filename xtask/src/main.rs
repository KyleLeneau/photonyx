//! See <https://github.com/matklad/cargo-xtask/>.
//!
//! This binary defines various auxiliary build commands, which are not
//! expressible with just `cargo`.
//!
//! This binary is integrated into the `cargo` command line by using an alias in
//! `.cargo/config`.

mod check;
mod codegen;
mod flags;

use std::{env, path::PathBuf};
use xshell::Shell;

fn main() -> anyhow::Result<()> {
    let flags = flags::Xtask::from_env_or_exit();

    let sh = &Shell::new()?;
    sh.change_dir(project_root());

    match flags.subcommand {
        flags::XtaskCmd::Check(cmd) => cmd.run(sh),
        flags::XtaskCmd::ExportSirilCommands(cmd) => cmd.run(sh),
        flags::XtaskCmd::MergeSirilCommands(cmd) => cmd.run(sh),
    }
}

/// Returns the path to the root directory of `photonyx` project.
fn project_root() -> PathBuf {
    let dir =
        env::var("CARGO_MANIFEST_DIR").unwrap_or_else(|_| env!("CARGO_MANIFEST_DIR").to_owned());
    PathBuf::from(dir).parent().unwrap().to_owned()
}
