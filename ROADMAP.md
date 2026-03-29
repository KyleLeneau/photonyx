## Roadmap

### Immediate

* [x] Updated readme
* [x] Updated LICENSE
* [x] Gitops / CI / CD
* [x] How to contribute
* [x] Clippy rules
* [x] docs / adr directory
* [x] Agents/Claude init
* [x] logging / tracing crate and setup
* [x] clap to photonyx cli front end for testing more things
* [x] siril-sys
* [x] Windows support & check / info command for siril testing
* [x] Figure out a builder pattern for command definition
* [x] github releases and install script (`cargo-dist`)
* [x] conside changing main crate to lib with /bin folder and named `px` instead
* [x] create and move cli definition to `px-cli` crate, leave commands in `px`
* [x] siril-commands
* [x] xtask to generate siril-commands
* [x] need to directory from the siril builder exposed to work inside of it later
* [x] need the best_rejection helper to sink some of the settings
* [x] maybe code generate an impl SirilExt for all commands to appear first class (accept builder)
* [x] resources should have a container aware memory and cpu limits
* [x] cargo-dist should install to usr/bin common folders like uv does
* [x] document git tagging and release process with `cargo-dist`
* [ ] fits crate to wrap some other fits library (cfitsio?)
* [ ] Siril workflows idea (jobs & workflows) like CI but for the processing engine
* [ ] need the conversion file helper to move files someplace else on the system
* [ ] need some type of processing_engine with the basic workflows
* [ ] the stdout and error streams from siril need to be exposed someplace so I consume elsewhere (like sse)
* [ ] `clap_complete` for shell completion
* [ ] self updating cli (like uv `px self update`) (https://docs.rs/axoupdater)
* [ ] need a `.px` folder in the user home initialized at some point to store default or common config
* [ ] need to support and test multi pipes on windows now that MR is merged for support
* [ ]


### Future

* [ ] need a better way to bump versions for a release
