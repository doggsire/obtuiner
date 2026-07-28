# obtuiner

A Rust workspace that bundles three Linux-focused terminal tools behind one CLI, plus an external plugin system for adding more:

- installer: search, install, and uninstall packages
- launcher: manage and launch app profiles; type `>` to search and run PATH commands
- updater: run full-system or per-package updates

The root binary dispatches to one of those built-in tools:

- installer or -i
- launcher or -l
- updater or -u

It also discovers and dispatches to installed plugins, such as the bundled `powermenu`:

- powermenu or -p: shutdown, reboot, sleep, logout

See [Plugins](#plugins) for how plugin discovery works and how to write your own.

## Features

- TUI-based workflows built with crossterm and ratatui
- Automatic package manager detection
- Supports native managers:
  - pacman (+ optional AUR helper: paru/yay)
  - apt or nala
  - dnf
  - zypper
- Flatpak integration when available
- Dry-run mode for installer/updater command preview
- Launcher profile persistence to JSON
- Launcher command mode: prefix query with `>` to search PATH executables and run them as `>command [args...]`
- External plugin system: any `obtuiner-*` executable on `PATH` (or in the plugins directory) can register itself as a tool
- powermenu plugin: a shutdown/reboot/sleep/logout menu using the same TUI controls as the built-in tools

## Workspace Layout

- obtuiner: top-level CLI entry point, built-in dispatcher, and plugin discovery
- installer: package browser/install/uninstall TUI
- launcher: launch profile manager and app launcher TUI
- updater: update task browser and executor TUI
- powermenu: shutdown/reboot/sleep/logout plugin (built as `obtuiner-powermenu`)
- plugin_api: shared metadata contract used by the root CLI and plugin executables
- runtime_ops: package manager detection and command/query logic
- core_domain: shared models and persistence helpers
- tui_kit: shared TUI rendering and key handling

## Install

### One-liner (recommended)

```bash
curl -fsSL https://raw.githubusercontent.com/doggsire/obtuiner/main/install.sh | sh
```

Or download and inspect before running:

```bash
curl -fsSL https://raw.githubusercontent.com/doggsire/obtuiner/main/install.sh -o install.sh
less install.sh
sh install.sh
```

The script:
- Detects your CPU architecture (x86\_64, aarch64)
- Downloads the prebuilt binary and its SHA256 checksum from the [latest release](https://github.com/doggsire/obtuiner/releases/latest)
- Verifies the checksum before installing
- Installs to `/usr/local/bin` (or `~/.local/bin` if `/usr/local/bin` is not writable)

After install, run from anywhere:

```bash
obtuiner installer
obtuiner launcher
obtuiner updater
```

### Manual binary install

Download the archive for your platform from the [latest release](https://github.com/doggsire/obtuiner/releases/latest), verify the checksum, and copy the binary to your PATH:

```bash
# Example for x86_64
ARCHIVE=obtuiner-vX.Y.Z-x86_64-unknown-linux-gnu.tar.gz
curl -LO "https://github.com/doggsire/obtuiner/releases/latest/download/$ARCHIVE"
curl -LO "https://github.com/doggsire/obtuiner/releases/latest/download/$ARCHIVE.sha256"
sha256sum -c "$ARCHIVE.sha256"
tar -xzf "$ARCHIVE"
sudo mv obtuiner /usr/local/bin/
```

### Build from source

Requires a [Rust toolchain](https://rustup.rs).

```bash
git clone https://github.com/doggsire/obtuiner.git
cd obtuiner
cargo build --release
sudo cp target/release/obtuiner /usr/local/bin/
```

Or install into `~/.cargo/bin` directly:

```bash
cargo install --path obtuiner
```

## Requirements

- Linux environment
- At least one supported package manager installed

Optional but recommended:

- sudo privileges for system package operations
- flatpak for Flatpak install/update support
- paru or yay for AUR support on Arch-based systems

## Run

If installed (via release binary or install script), run through the unified CLI:

```bash
obtuiner installer
obtuiner launcher
obtuiner updater
obtuiner powermenu
```

Short aliases:

```bash
obtuiner -i
obtuiner -l
obtuiner -u
obtuiner -p
```

Dry-run mode (where supported):

```bash
obtuiner installer --dry-run
obtuiner updater --dry-run
```

### Run from source (development)

If you are working from a local clone without installing the binary, use Cargo:

```bash
cargo run -p obtuiner -- installer
cargo run -p obtuiner -- launcher
cargo run -p obtuiner -- updater
```

Short aliases:

```bash
cargo run -p obtuiner -- -i
cargo run -p obtuiner -- -l
cargo run -p obtuiner -- -u
```

Dry-run mode (where supported):

```bash
cargo run -p obtuiner -- installer --dry-run
cargo run -p obtuiner -- updater --dry-run
```

`powermenu` is a plugin rather than a built-in, so running it from source means building it and putting it on `PATH` first — see [Plugins](#plugins).

## Usage Help

If no subcommand is provided, the binary prints usage, plus any discovered plugins:

```text
Usage: obtuiner <installer|launcher|updater|-i|-l|-u> [args...]

Discovered plugins:
  powermenu (-p) - Power menu: shutdown, reboot, sleep, logout
```

## Plugins

Obtuiner supports external plugin executables in addition to its built-in tools. A plugin is any executable whose file name starts with `obtuiner-` (for example `obtuiner-powermenu`), found either on `PATH` or in Obtuiner's plugin directory:

- `~/.local/share/ui/plugins/`

When an unrecognized subcommand is given, the root CLI scans those locations and asks each `obtuiner-*` executable to identify itself by invoking it with a metadata flag:

```bash
obtuiner-powermenu --obtuiner-plugin-metadata
# {"name":"powermenu","aliases":["-p"],"summary":"Power menu: shutdown, reboot, sleep, logout"}
```

A valid plugin responds by printing a single-line JSON document (`name`, `aliases`, `summary`) to stdout and exiting successfully. Once discovered, the plugin is resolved by its `name` or any of its `aliases`, and invoked with the same arguments and stdio as a built-in tool — so it can freely draw its own TUI.

The `plugin_api` crate defines this contract (`PluginMetadata`, the metadata flag, and the `obtuiner-` naming prefix) and provides `handle_metadata_handshake()` for plugin authors to call at the top of `main()`.

### Building and installing the powermenu plugin

```bash
cargo build --release -p powermenu
cp target/release/obtuiner-powermenu ~/.local/bin/   # or anywhere on PATH
```

Then run it via the root CLI:

```bash
obtuiner powermenu
obtuiner -p
```

powermenu's four actions (shutdown, reboot, sleep, logout) use `hyprshutdown` when it's available on `PATH` — for a graceful logout, and as the `--post-cmd` step after shutdown/reboot — falling back to direct `systemctl`/`loginctl` commands otherwise. `sleep` always runs `systemctl suspend`.

### Writing your own plugin

1. Build an executable named `obtuiner-<yourtool>`.
2. At the top of `main()`, check for `plugin_api::METADATA_FLAG` and respond with your `PluginMetadata` as JSON (or call `plugin_api::handle_metadata_handshake`).
3. Otherwise, run your tool normally using whatever arguments were passed through.
4. Put the executable on `PATH` or in `~/.local/share/ui/plugins/`.

For a real TUI plugin, `tui_kit` provides the same shared search/results/details layout, key handling, and confirmation modal used by the built-in tools, so a plugin can match Obtuiner's look and controls exactly — see the `powermenu` crate for a complete example.

## Data Storage

Launcher profiles are stored under XDG config paths:

- ~/.config/ui/launcher/profiles.json

If no profiles are present, defaults are generated (for example Terminal and VS Code).

Discovered plugin executables are looked up from:

- ~/.local/share/ui/plugins/ (in addition to `PATH`)

## Tests

Run all workspace tests:

```bash
cargo test --workspace
```

## Notes

- This project is currently Linux-oriented.
- Command execution depends on tools available on your machine.
- Some package actions may prompt for credentials depending on your sudo configuration.
