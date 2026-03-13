# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**Photonyx** is a Rust CLI tool for astrophotography processing built on top of [Siril 1.4.0](https://www.siril.org/). It is in early development — the workspace and CI infrastructure are established, but the core features (Siril bindings, command generation) are still being built.

## Commands

```bash
# Build
cargo build

# Run the photonyx binary
cargo px [args]           # alias for: cargo run --package photonyx --bin photonyx --

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
cargo test --package photonyx <test_name>
```

## Architecture

This is a Cargo workspace (`resolver = "2"`, `edition = "2024"`, MSRV 1.91) with three member groups:

- **`crates/`** — Application binaries. Currently contains `photonyx` (the main CLI binary).
- **`lib/`** — Shared library crates. Currently empty, intended for future `siril-sys` (low-level FFI bindings) and `siril-commands` (high-level command abstractions) crates.
- **`xtask/`** — Build automation via the [xtask pattern](https://github.com/matklad/cargo-xtask). Flags are defined in `xtask/src/flags.rs` using `xflags`.

The architecture follows the [rust-analyzer workspace pattern](https://matklad.github.io/2021/08/22/large-rust-workspaces.html). Common dependencies are declared at the workspace root in `[workspace.dependencies]` and referenced in member crates with `{ workspace = true }`.

## Roadmap Context

Near-term planned work (see `ROADMAP.md`):
- `siril-sys` crate: low-level Siril bindings
- `siril-commands` crate: high-level command abstractions
- `xtask` command to generate `siril-commands`

Architecture decisions are documented as ADRs in `docs/adr/`. Design documents are in `docs/designs/`.

## Conventions

- **Commit style:** Conventional commits (`feat:`, `fix:`, `chore:`)
- **Line length:** 100 characters max
- **Indentation:** 4 spaces for Rust, 2 for YAML/Markdown
