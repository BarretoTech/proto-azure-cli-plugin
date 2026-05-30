# proto-azure-cli-plugin

A [proto](https://moonrepo.dev/proto) WASM plugin for managing
[Azure CLI](https://github.com/Azure/azure-cli) installations.

Per-version, side-by-side installs of `az` managed by proto — without touching
your system package manager.

## Installation

Add to `.prototools`:

```toml
[plugins]
azure-cli = "github://BarretoTech/proto-azure-cli-plugin"
```

Or with a pinned plugin version:

```toml
[plugins]
azure-cli = "github://BarretoTech/proto-azure-cli-plugin@v0.1.0"
```

## Usage

Install the latest version:

```bash
proto install azure-cli
```

Install a specific version:

```bash
proto install azure-cli 2.86.0
```

List available versions:

```bash
proto versions azure-cli
```

Pin a version in `.prototools`:

```bash
proto pin azure-cli 2.86.0
```

After installation, the `az` command is available:

```bash
az --version
az login
```

## Supported Platforms

| Platform          | Architecture | Install strategy                                   |
| ----------------- | ------------ | -------------------------------------------------- |
| macOS             | arm64, x64   | Prebuilt tarball from GitHub (bundles Python 3.13) |
| Windows           | x64          | Prebuilt ZIP from GitHub (bundles Python 3.13)     |
| Linux (any libc)  | x64, arm64   | `pip install` into a per-version virtualenv         |
| Windows           | arm64        | Not supported                                      |

### Linux prerequisites

The Linux install path needs:

- **Python 3.10 or newer** on `PATH` as `python3`
- The **`venv` module** — usually a separate package on Debian/Ubuntu
  (`sudo apt install python3-venv` or `python3.10-venv`), built-in elsewhere
- Network access to PyPI for `pip install azure-cli==<version>`

The plugin will surface a clear error if any of these are missing.

## Configuration

Override the archive mirror URL in `.prototools`:

```toml
[tools.azure-cli]
dist-url = "https://your-mirror.example.com/azure-cli-releases"
```

The default is `https://github.com/Azure/azure-cli/releases/download`. The
plugin appends `/azure-cli-<version>/<asset-name>` to this URL.

## How it works

Azure CLI is a Python application, not a single static binary, so the plugin
dispatches by host OS:

- **macOS + Windows**: Microsoft publishes self-contained archives on GitHub
  (`azure-cli-<v>-macos-{arm64,x86_64}.tar.gz`, `azure-cli-<v>-x64.zip`) that
  bundle their own Python 3.13 runtime. The plugin downloads and unpacks the
  archive matching your platform.
- **Linux**: no portable Microsoft artifact exists, so the plugin uses
  `proto`'s native install hook to create a per-version `venv` with your
  system Python and `pip install azure-cli==<version>` into it. The result
  is a fully isolated install at `~/.proto/tools/azure-cli/<version>/venv/`.

In both cases proto's shim system exposes the `az` command on your `PATH`.

## Development

Setup toolchain:

```bash
proto install rust
rustup target add wasm32-wasip1
```

Build the plugin:

```bash
cargo build --target wasm32-wasip1
```

Test locally with proto by pointing at the built WASM:

```toml
# .prototools
[plugins]
azure-cli = "file://./target/wasm32-wasip1/debug/azure_cli_plugin.wasm"
```

```bash
proto --log trace install azure-cli
proto --log trace versions azure-cli
az --version
```

## License

MIT
