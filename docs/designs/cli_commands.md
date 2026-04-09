# Draft of px commands

## Concepts

* Profiles are HW profiles and a directory on disk with images
* Projects are linear stacks and color produced images composed (what you share to the world)
* Calibration Master are master frames used to calibrate light frames, associated with HW

### Manage install of binary

```bash
px self version
px self update
px help
```

### Inspect a file
```bash
px inspect <file>
```

### Manage HW profiles
```bash
px profile          # <alias to show?>
px profile list     # list all profiles knonw to system
px profile init     # make a new profile (dir, layout, config, etc)
px profile show     # show the current profile or a specified one
px profile scan     # scan for changes in current or specified profile
```

### Manage Data / Remote
```bash
px remote add       # add a new remote
px remote delete    # delete an added remote
px remote list      # list all the remotes configured (like rclone)
px remote push      # push data to a remote like to a NAS
px remote pull      # pull data from remote like sfro computer

px remote monitor   # some sort of profile monitoring UI?
```

### Manage Calibration master
```bash
px master best      # find best master <type> based on query
px master list      # show all the master for a profile
px master bias      # create a new master bias for profile
px master dark      # create a new master dark for profile
px master flat      # create a new master flat for profile
```

### Manage Projects
```bash
px project init     # new project setup
px project add      # add an observation to the project
px project list     # list all the projects for the profile
px project calibrate  # calibrate new light frames
px project stack    # linear stack by filter
px project align    # aka register a bunch of linear stacks
px project sample   # produce color samples using linear stacks

px project live     # start producing a livestack by watching for observations
```

### Manage Observations (Lights) (alias: obs)
```bash
px observation list
px observation calibrate        # calibrate a set of raw light frames

# Other ideas?
cull      # some kind of UI to remove bad frames
blink     # video of frames to reject
sample    # autostretch and produce sample to show (or preview)
thumbnail # produce thumbnails of all the images
```

### Monitor
```bash
px monitor
```

### Web App
```bash
px serve            # serve up a webui
```





```
An extremely fast Python package manager.

Usage: uv [OPTIONS] <COMMAND>

Commands:
  auth     Manage authentication
  run      Run a command or script
  init     Create a new project
  add      Add dependencies to the project
  remove   Remove dependencies from the project
  version  Read or update the project's version
  sync     Update the project's environment
  lock     Update the project's lockfile
  export   Export the project's lockfile to an alternate format
  tree     Display the project's dependency tree
  format   Format Python code in the project
  tool     Run and install commands provided by Python packages
  python   Manage Python versions and installations
  pip      Manage Python packages with a pip-compatible interface
  venv     Create a virtual environment
  build    Build Python packages into source distributions and wheels
  publish  Upload distributions to an index
  cache    Manage uv's cache
  self     Manage the uv executable
  help     Display documentation for a command

Cache options:
  -n, --no-cache               Avoid reading from or writing to the cache, instead using a temporary directory for the duration of the operation [env: UV_NO_CACHE=]
      --cache-dir <CACHE_DIR>  Path to the cache directory [env: UV_CACHE_DIR=]

Python options:
      --managed-python       Require use of uv-managed Python versions [env: UV_MANAGED_PYTHON=]
      --no-managed-python    Disable use of uv-managed Python versions [env: UV_NO_MANAGED_PYTHON=]
      --no-python-downloads  Disable automatic downloads of Python. [env: "UV_PYTHON_DOWNLOADS=never"]

Global options:
  -q, --quiet...                                   Use quiet output
  -v, --verbose...                                 Use verbose output
      --color <COLOR_CHOICE>                       Control the use of color in output [possible values: auto, always, never]
      --native-tls                                 Whether to load TLS certificates from the platform's native certificate store [env: UV_NATIVE_TLS=]
      --offline                                    Disable network access [env: UV_OFFLINE=]
      --allow-insecure-host <ALLOW_INSECURE_HOST>  Allow insecure connections to a host [env: UV_INSECURE_HOST=]
      --no-progress                                Hide all progress outputs [env: UV_NO_PROGRESS=]
      --directory <DIRECTORY>                      Change to the given directory prior to running the command [env: UV_WORKING_DIR=]
      --project <PROJECT>                          Run the command within the given project directory [env: UV_PROJECT=]
      --config-file <CONFIG_FILE>                  The path to a `uv.toml` file to use for configuration [env: UV_CONFIG_FILE=]
      --no-config                                  Avoid discovering configuration files (`pyproject.toml`, `uv.toml`) [env: UV_NO_CONFIG=]
  -h, --help                                       Display the concise help for this command
  -V, --version                                    Display the uv version
```
