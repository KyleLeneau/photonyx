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
* [ ] siril-commands
* [ ] xtask to generate siril-commands
* [ ] fits crate to wrap some other fits library (cfitsio?)
* [ ] Siril workflows idea (jobs & workflows) like CI but for the processing engine
* [ ] need to directory from the siril builder exposed to work inside of it later
* [ ] need the conversion file helper to move files someplace else on the system
* [ ] need the best_rejection helper to sink some of the settings
* [ ] need some type of processing_engine with the basic workflows
* [ ] resources should have a container aware memory and cpu limits
* [ ] the stdout and error streams from siril need to be exposed someplace so I consume elsewhere (like sse)
* [ ] maybe code generate an impl SirilExt for all commands to appear first class (accept builder)
* [ ] need to test multi pipe on windows now that MR is merged for support
* [ ] clap_complete for shell completion
* [ ] document git tagging and release process with `cargo-dist`
* [ ] self updating cli (like uv `px self update`) (https://docs.rs/axoupdater)
* [ ] conside changing main crate to lib with /bin folder and named `px` instead
* [ ] create and move cli definition to `px-cli` crate, leave commands in `px`
* [ ]


### Future
