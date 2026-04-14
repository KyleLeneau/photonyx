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
