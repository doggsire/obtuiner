# obtuiner

Development note: AI tools were used during the creation of this app.

A Rust workspace that bundles three Linux-focused terminal tools behind one CLI:

- installer: search, install, and uninstall packages
- launcher: manage and launch app profiles; type `>` to search and run PATH commands
- updater: run full-system or per-package updates

The root binary dispatches to one of those tools:

- installer or -i
- launcher or -l
- updater or -u

## Features

- TUI-based workflows built with crossterm and ratatui
- Automatic package manager detection
- Supports native managers:
  - pacman (+ optional AUR helper: paru/yay)
  - apt (nala optional)
  - dnf
  - zypper
- Flatpak integration when available
- Dry-run mode for installer/updater command preview
- Launcher profile persistence to JSON
- Launcher command mode: prefix query with `>` to search PATH executables and run them as `>command [args...]`

## Workspace Layout

- obtuiner: top-level CLI entry point and dispatcher
- installer: package browser/install/uninstall TUI
- launcher: launch profile manager and app launcher TUI
- updater: update task browser and executor TUI
- runtime_ops: package manager detection and command/query logic
- core_domain: shared models and persistence helpers
- tui_kit: shared TUI rendering and key handling

## Install

### One-liner (recommended)

```bash
curl -fsSL https://github.com/doggsire/obtuiner/releases/latest/download/install.sh | sh
```

Or download and inspect before running:

```bash
curl -fsSL https://github.com/doggsire/obtuiner/releases/latest/download/install.sh -o install.sh
less install.sh
sh install.sh
```

The script:
- Detects your CPU architecture (x86\_64, aarch64)
- Downloads the prebuilt binary and its SHA256 checksum from the [latest release](https://github.com/doggsire/obtuiner/releases/latest)
- Verifies the checksum before installing
- Installs to `/usr/local/bin` (or `~/.local/bin` if `/usr/local/bin` is not writable)

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

After install, run from anywhere:

```bash
obtuiner installer
obtuiner launcher
obtuiner updater
```

## Requirements

- Linux environment
- At least one supported package manager installed

Optional but recommended:

- sudo privileges for system package operations
- nala for apt-based systems, if you prefer it over apt
- flatpak for Flatpak install/update support
- paru or yay for AUR support on Arch-based systems

## Run

After installing from source, run through the unified CLI:

```bash
obtuiner installer
obtuiner launcher
obtuiner updater
```

Short aliases:

```bash
obtuiner -i
obtuiner -l
obtuiner -u
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

## Usage Help

If no subcommand is provided, the binary prints:

```text
Usage: obtuiner <installer|launcher|updater|-i|-l|-u> [args...]
```

## Data Storage

Launcher profiles are stored under XDG config paths:

- ~/.config/ui/launcher/profiles.json

If no profiles are present, defaults are generated (for example Terminal and VS Code).

## Tests

Run all workspace tests:

```bash
cargo test --workspace
```

## Notes

- This project is currently Linux-oriented.
- AI tools were used during the creation of this app.
- Command execution depends on tools available on your machine.
- Some package actions may prompt for credentials depending on your sudo configuration.

## License

Copyright 2026 doggsire

Licensed under the [Apache License, Version 2.0](LICENSE). You may not use this project except in compliance with the License. See the [NOTICE](NOTICE) file for attribution details.
