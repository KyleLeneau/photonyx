# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Photonyx** is a Rust CLI tool for astrophotography processing built on top of [Siril 1.4.0](https://www.siril.org/). It is in early development — the workspace, CI/CD, and low-level Siril bindings (`siril-sys`) are established; higher-level command abstractions and processing workflows are still being built.

## Commands

```bash
# Build
cargo build

# Run the photonyx binary
cargo px [args]           # alias for: cargo run --package px --bin px --

# Run tests
cargo test

# Lint
cargo lint                # alias for: cargo clippy --all-targets -- --cap-lints warn

# Format
cargo fmt

# Run xtask build automation
cargo xtask [args]        # alias for: cargo run --package xtask --bin xtask --

# Run a single test (Rust standard pattern)
cargo test <test_name>
cargo test --package px <test_name>
```

## Architecture

This is a Cargo workspace (`resolver = "2"`, `edition = "2024"`, MSRV 1.91) with three member groups:

- **`crates/`** — Application and library crates. Currently contains:
  - `px` — The main CLI binary (commands and runtime logic).
  - `px-cli` — CLI definition only (clap structs/enums), separated to allow reuse without pulling in all dependencies.
  - `siril-sys` — Low-level Siril bindings: spawns the `siril` process, communicates via named pipe, and provides typed command builders.
- **`lib/`** — Reserved for future shared library crates (currently empty).
- **`xtask/`** — Build automation via the [xtask pattern](https://github.com/matklad/cargo-xtask). Flags are defined in `xtask/src/flags.rs` using `xflags`.

The architecture follows the [rust-analyzer workspace pattern](https://matklad.github.io/2021/08/22/large-rust-workspaces.html). Common dependencies are declared at the workspace root in `[workspace.dependencies]` and referenced in member crates with `{ workspace = true }`.

## Roadmap Context

Completed work (see `ROADMAP.md` for full history):
- `siril-sys` crate with typed command builders and named-pipe Siril communication
- `px-cli` crate separating CLI definitions from command execution
- CI/CD via `cargo-dist`, Windows support, logging/tracing setup

Near-term planned work:
- `siril-commands` crate: high-level command abstractions
- `xtask` command to generate `siril-commands`
- `fits` crate wrapping a fits library (e.g. cfitsio)
- Siril workflows (jobs & workflows, processing engine)

Architecture decisions are documented as ADRs in `docs/adr/`. Design documents are in `docs/designs/`.

## Conventions

- **Commit style:** Conventional commits (`feat:`, `fix:`, `chore:`)
- **Line length:** 100 characters max
- **Indentation:** 4 spaces for Rust, 2 for YAML/Markdown
