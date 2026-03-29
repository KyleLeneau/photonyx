# photonyx - _(pho-ton-ix)_

![Static Badge](https://img.shields.io/badge/rust-red?logo=rust)
[![Actions status](https://github.com/KyleLeNeau/photonyx/actions/workflows/ci.yml/badge.svg)](https://github.com/KyleLeNeau/photonyx/actions)

Photonyx is command line application that uses conventions and configuration files to perform photometry and astrophotography processing. It is built on top of [Siril 1.4.0](https://www.siril.org/).

## Features

* TBD

## Requirements

* Siril installed on your system (https://www.siril.org/)
* cargo & rust (https://rust-lang.org/tools/install/)

## Installation

Windows, Mac, Linux - use the install script from an available [release](https://github.com/KyleLeneau/photonyx/releases)

If installed via the standalone installer, px can update itself to the latest version:

```bash
px self update
```

### Shell Completions

Shell completion are available, run one of the following (use `echo $SHELL` to determine your shell):

#### bash
```bash
echo 'eval "$(px generate-shell-completion bash)"' >> ~/.bashrc
```

#### zsh
```bash
echo 'eval "$(px generate-shell-completion zsh)"' >> ~/.zshrc
```

#### fish
```bash
echo 'px generate-shell-completion fish | source' > ~/.config/fish/completions/px.fish
```

#### Elvish
```bash
echo 'eval (px generate-shell-completion elvish | slurp)' >> ~/.elvish/rc.elv
```

#### PowerShell / pwsh
```powershell
if (!(Test-Path -Path $PROFILE)) {
  New-Item -ItemType File -Path $PROFILE -Force
}
Add-Content -Path $PROFILE -Value '(& px generate-shell-completion powershell) | Out-String | Invoke-Expression'
```

## Uninstallation

Remove the px binary:

#### macOS and Linux
```console
$ rm ~/.local/bin/px
```

#### Windows
```pwsh-session
PS> rm $HOME\.local\bin\px.exe
```

## Usage

TBD

## Roadmap

Please see [ROADMAP.md](./ROADMAP.md) for more details.

## Contributing

PRs are welcome & appreciated! See the [contributing guide](./CONTRIBUTING.md) to get started.

## FAQ

TBD

## Acknowledgements

1. mono-repo layout inspired by [Rust Workspaces](https://matklad.github.io/2021/08/22/large-rust-workspaces.html) and [rust-analyzer](https://github.com/rust-lang/rust-analyzer)
2. [Siril](https://siril.org/)
3. [async-siril](https://github.com/KyleLeneau/async-siril)

## License

Photonyx is licensed under:

- BSD-3-Clause license ([LICENSE](LICENSE) or <https://opensource.org/licenses/BSD-3-Clause>)

Unless you explicitly state otherwise, any contribution intentionally submitted for inclusion in Photonyx by you, as defined in the BSD-3-Clause license, shall be dually licensed as above, without any additional terms or conditions.
