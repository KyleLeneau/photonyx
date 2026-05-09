# Changelog

<!-- prettier-ignore-start -->

## 0.1.4

* Add shell completion, use `px generate-shell-completion zsh` to generate
* Updated README.md with install, completiong and uninstall instructions
* Add an `OutputSink` to siril builder to redirect stdout/stderr
* Removed `stat` command and replaced with `inspect`
* Turned off logging by default and moved to `-v, -vv, -vvv` global cli options
* Add `px master bias` implementation to create master bias
* Add `px master dark` implementation to create master bias
* Added master file naming convention
* Add `px master flat` implementation to create master flat
* Add `ConversionFile` type to siril-sys to track file movements with siril
* Add `px observation calibrate` implementation for processing light frames
* Add `px project init` to create a folder and config file
* Add `px project add` to add an observation to the project and start filling out linear stack creation
* Add `px project stack` and basic implementation with a number of todo's
* Add `px profile init` and a convention plus profile config file
* Enforce profile path being specified on `px master ..` commands
* Validate that siril is installed on select commands and give instructions if it's not
* Change the master bias to use `px-pipeline` crate
* Remove the lifetime on `siril-sys::Builder`
* Change master dark command to use `px-pipeline`
* Add common master file types to `px-fits`
* Change master flat to use `px-pipeline`
* Created a general pipeline reporter for cli progress
* Fixed `px profile init` to do an import if `BIAS, DARK, FLAT, LIGHT` exist with no config
* Added sqlite DB for master and observation storage in `.px/index.db`
* Added new `px-index` crate to interact with DB
* Implemented `px master list` to show everything in the DB
* Add output format to `px master list` to render as json
* Ensure that the passed in filter to `px master flat` ends up in the resulting master fits header
* Move siril calibrate steps into `px-pipeline`
* Added observation to ProfileIndex on completion
* Reworked the meta models in `px-fits` and how headers and capture dates were parsed
* Change `--bias` to be optional on `px master flat` and allow the bias to be found in the index
* `px obs calibrate` no longer enforces bias, dark, or flat to support smart scopes that just need debayering
* Rework project config and implement `single` and `spiral_mosiac` stacking for projects
* Changed `px project init` to be interactive and build a project config.

## 0.1.3

Released on 2026-03-29.

* Empty release to test `px self update`

## 0.1.2

Released on 2026-03-29.

* Adding `px self update` to binary

## 0.1.1

Released on 2026-03-28.

* Second release to change install location

## 0.1.0

Released on 2026-03-20.

* Initial release using `cargo-dist`
