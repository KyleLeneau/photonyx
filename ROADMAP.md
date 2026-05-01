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
* [x] self updating cli (like uv `px self update`) (https://docs.rs/axoupdater)
* [x] `clap_complete` for shell completion
* [x] update readme on how to install, update and setup shell completion
* [x] the stdout and error streams from siril need to be exposed someplace so I consume elsewhere (like sse)
* [x] fits crate to wrap some other fits library
* [x] Add a "conventions" crate or similar for file naming conventions or directory layouts?
* [x] need the conversion file helper to move files someplace else on the system
* [x] explore building a gui based app that allows a user to "blink" their frames easily or view fits headers
* [x] Config file support and order loading so that a user can specify less on the command line for their profile
* [x] post-install or validation step to ensure Siril is installed and available with minimum version
* [x] need some type of processing_engine with the basic workflows
* [x] need a way to save/store the master calibration frames inside the profile so they can be referenced later on (should auto create this after each command)
* [x] need a way to import a profile into px
* [x] add a master list command that lists all active master, support json and tui display
* [x] move the calibrate obs steps into px-pipeline
* [x] move the linear stack creation into px-pipeline
* [ ] need to implement scan of observations so that an inventory of lights is managed and have a place to associate master calibration frames to
* [ ] implement a profile level or top level "watcher" that watches for file drops in light obs folder, scans, finds best master and sets that link up and then calibrate individual sub frames as they come in
* [ ] build on top of the per sub frame calibration and send to a livestacker (tasks & channels) that produces linear weighted stacks per target then send to another queue to beautify them with the latest beuty image added to a rolling motion movie stream
* [ ] build a command that monitors the advanced API of nina and renders a video of the guiding results along with a history of events on the right side
* [ ] simple web server to recieve events of where scopes are pointed (ra, dec, fov, pixel size, name) and render an event feed on a web UI, and display a bounding box on a stellarium view. Have the cli initialize and send this data through a LIGHT file watcher.
* [ ] need a debug tool to trigger the similar flow of a file watcher to simulate file drops by by "playing" a folder of files
* [ ] implement the pipelines for color samples
* [ ] implement obs find best to manage calibration links
* [ ] top level commands like `px scan` and `px calibrate` as shortcuts to do setup actions
* [ ] need a tool to bulk edit or ensure a filter name is added to all files in a sequence (for scopes that omit this keyword)

### Bugs

* [x] fix all places where relative paths could be passed in, they need to be resolved to full paths for Siril...
* [x] fix why the master file names don't see the YYYY-MM-DD format in the path string
* [x] better populate a Light frame into the observation_set table (ra, dec, date, target name)

### Future

* [ ] should the project config type exist in the DB instead?
* [ ] need to support and test multi pipes on windows now that MR is merged for support
* [ ] need a better way to bump versions for a release
* [ ] need a `.px` folder in the user home initialized at some point to store default or common config (https://crates.io/crates/etcetera) - can also use this for cross profile querying
