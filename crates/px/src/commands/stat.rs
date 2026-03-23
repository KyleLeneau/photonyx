use std::fmt::Write;

use anyhow::Result;
use siril_sys::Builder;

use crate::{commands::ExitStatus, printer::Printer};

pub(crate) async fn stat(file: &str, printer: Printer) -> Result<ExitStatus> {
    // Startup and wait till process is ready for additional commands
    let mut siril = Builder::default().build().await?;
    siril.command("requires 0.99.10").await?;
    siril.command("set core.mem_ratio=0.9").await?;
    siril.command(&format!("load {}", file)).await?;

    let stat_output = siril.command("stat").await;
    for line in &stat_output.unwrap() {
        writeln!(printer.stdout(), "stat: {:?}", line)?;
    }

    // TODO: Need a better way? to wait for enter key to continue so can check temp directories
    block_till_input();

    siril.close().await?;
    // Siril also cleans up when dropped

    Ok(ExitStatus::Success)
}

fn block_till_input() {
    use std::io::{self, BufRead};

    let stdin = io::stdin();
    stdin.lock().lines().next();
}
