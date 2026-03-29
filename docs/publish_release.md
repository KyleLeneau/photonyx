# Publish for Release

Using cargo-dist here is a PR from UV on how to test this: https://github.com/astral-sh/uv/pull/8420

## Steps

* Update [CHANGELOG.md](../CHANGELOG.md)
* Edit the versions in crates from `find . -name Cargo.toml`
* set publish version in `./crates/px/Cargo.toml` (user facing)
* run `dist plan` to see what will be made
* commit changes `git commit -am "release: 0.2.0"`
* tag for release `git tag "v0.2.0"`
* push tag to kick off CI/CD `git push origin v0.2.0`
*

## Debug Install

* build locally with `cargo build`
* `chmod +x ./target/distrib/px-installer.sh`
* host a web server with files `npx http-server -o ./target/distrib`
* install from hosted server: `PX_DOWNLOAD_URL="http://127.0.0.1:8080/target/distrib/" ./target/distrib/px-installer.sh`
* confirm what version exists and where `which px`

## Uninstall

* on mac: `rm ~/.local/bin/px`

## Test update

TODO:
