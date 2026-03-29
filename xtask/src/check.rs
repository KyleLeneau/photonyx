use anyhow::Ok;
use xshell::{Shell, cmd};

use crate::flags;

impl flags::Check {
    pub(crate) fn run(&self, sh: &Shell) -> Result<(), anyhow::Error> {
        if self.fmt {
            cmd!(&sh, "cargo fmt").run()?;
        }

        cmd!(&sh, "cargo test --all-features --verbose").run()?;
        cmd!(&sh, "cargo clippy --all-features --verbose").run()?;

        Ok(())
    }
}
