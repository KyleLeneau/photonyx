# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

* **Photonyx** is a Rust CLI tool for astrophotography processing built on top of [Siril](https://www.siril.org/). Shares similarities to `git` and `uv` tools.
* Build with `cargo build`
* Run tests with `cargo test --verbose`
* Run single tests with `cargo test <test_name>` or `cargo test --package px <test_name>`
* Run clippy / linting with `cargo clippy --verbose`
* Run the photonyx binary with `cargo px [args]`
* Be sure to format code with `cargo fmt`
* Build and local tool automation is done with xtask by calling `cargo xtask [args]`
* Project is a monorepo and multi crate architecture
* Common dependencies are declared at the workspace root in `[workspace.dependencies]` and referenced in member crates with `{ workspace = true }`.
* Architecture decisions are documented as ADRs in `docs/adr/`. Design documents are in `docs/designs/`.

## Conventions
- **Commit style:** Conventional commits (`add:`, `fix:`, `chore:`),
- **Line length:** 100 characters max
- **Indentation:** 4 spaces for Rust, 2 for YAML/Markdown
