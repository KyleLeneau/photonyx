use std::fmt::Write as _;

use anyhow::Result;
use owo_colors::OwoColorize;
use px_cli::{CalibrationImageType, ListMasterArgs};
use px_index::{CalibrationSetRow, MasterKind, ProfileIndex};

use crate::{ExitStatus, printer::Printer};

pub(crate) async fn list_masters(
    args: ListMasterArgs,
    printer: Printer,
    index: ProfileIndex,
) -> Result<ExitStatus> {
    // Map CLI filter args to MasterKind; empty = all.
    let kinds: Vec<MasterKind> = args.image_type.iter().map(cli_kind_to_master_kind).collect();

    let rows = if kinds.len() == 1 {
        index.list_masters(Some(kinds[0])).await?
    } else {
        index.list_masters(None).await?
    };

    // If multiple kinds were specified, filter in-memory after fetching all.
    let rows: Vec<&CalibrationSetRow> = if kinds.len() > 1 {
        rows.iter()
            .filter(|r| kinds.iter().any(|k| k.as_str() == r.kind))
            .collect()
    } else {
        rows.iter().collect()
    };

    if rows.is_empty() {
        printer.info("no masters registered in this profile")?;
        return Ok(ExitStatus::Success);
    }

    let mut out = printer.stdout();

    // Header
    writeln!(
        out,
        " {:<5}  {:<10}  {:<6}  {:<8}  {:<7}  {:<6}  {}",
        "KIND".bold(),
        "DATE".bold(),
        "FILTER".bold(),
        "EXPOSURE".bold(),
        "TEMP °C".bold(),
        "GAIN".bold(),
        "MASTER PATH".bold(),
    )?;
    writeln!(
        out,
        " {:-<5}  {:-<10}  {:-<6}  {:-<8}  {:-<7}  {:-<6}  {:-<11}",
        "", "", "", "", "", "", ""
    )?;

    for row in rows {
        let filter = row.filter.as_deref().unwrap_or("–");
        let exposure = row
            .exposure
            .map(|e| format!("{e}s"))
            .unwrap_or_else(|| "–".to_string());
        let temp = row
            .temperature
            .map(|t| format!("{t:.1}"))
            .unwrap_or_else(|| "–".to_string());
        let gain = row
            .gain
            .map(|g| g.to_string())
            .unwrap_or_else(|| "–".to_string());

        let kind_colored = match row.kind.as_str() {
            "bias" => row.kind.cyan().to_string(),
            "dark" => row.kind.yellow().to_string(),
            "flat" => row.kind.green().to_string(),
            _ => row.kind.clone(),
        };

        writeln!(
            out,
            " {:<5}  {:<10}  {:<6}  {:<8}  {:<7}  {:<6}  {}",
            kind_colored,
            row.date,
            filter,
            exposure,
            temp,
            gain,
            row.master_path,
        )?;
    }

    Ok(ExitStatus::Success)
}

fn cli_kind_to_master_kind(k: &CalibrationImageType) -> MasterKind {
    match k {
        CalibrationImageType::Bias => MasterKind::Bias,
        CalibrationImageType::Dark => MasterKind::Dark,
        CalibrationImageType::Flat => MasterKind::Flat,
    }
}
