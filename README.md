# obtuiner

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
  - apt or nala
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

## Requirements

- Rust toolchain (stable) with Cargo
- Linux environment
- At least one supported package manager installed

Optional but recommended:

- sudo privileges for system package operations
- flatpak for Flatpak install/update support
- paru or yay for AUR support on Arch-based systems

## Build

From repository root:

```bash
cargo build
```

Release build:

```bash
cargo build --release
```

## Install To bin

Install the CLI into your Cargo bin directory (usually ~/.cargo/bin):

```bash
cargo install --path obtuiner
```

After install, run it from anywhere:

```bash
obtuiner installer
obtuiner launcher
obtuiner updater
```

Short forms:

```bash
obtuiner -i
obtuiner -l
obtuiner -u
```

If obtuiner is not found, add Cargo bin to your PATH:

```bash
export PATH="$HOME/.cargo/bin:$PATH"
```

Persist that in your shell profile (~/.bashrc or ~/.zshrc) and reload the shell.

Optional: install to a custom root (creates <root>/bin/obtuiner):

```bash
cargo install --path obtuiner --root /some/custom/prefix
```

## Run

Run through the unified CLI:

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
- Command execution depends on tools available on your machine.
- Some package actions may prompt for credentials depending on your sudo configuration.
