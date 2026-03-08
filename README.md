# Clipboard Manager

A lightweight, terminal-based clipboard manager for Linux built with Rust. Designed for tiling WMs like Hyprland.

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Docs](https://img.shields.io/badge/docs-deepwiki-purple.svg)](https://deepwiki.com/Grenish/clipboard-manager)

## Features

- Clipboard history (last 50 entries, text & images)
- Smart deduplication — re-copied content moves to top
- Persistent history across reboots
- Auto-detection of Hyprland with floating window rules
- Background daemon + `ratatui` TUI
- Wayland (`wl-clipboard`) and X11 support

## Installation

### Quick Install

```bash
curl -fsSL https://raw.githubusercontent.com/Grenish/clipboard-manager/main/install.sh | bash
```

To install a specific version:

```bash
VERSION=v0.1.5 curl -fsSL https://raw.githubusercontent.com/Grenish/clipboard-manager/main/install.sh | bash
```

### AUR (Arch Linux)
> Outdated

```bash
yay -S clipboard-manager-rs-git
```

### From Source

```bash
git clone https://github.com/Grenish/clipboard-manager.git
cd clipboard-manager
cargo install --path .
```

> Wayland users need `wl-clipboard` installed. Auto-paste is not yet implemented.

## Usage

**1. Start the daemon** (add to your WM startup):

```bash
clipboard-manager &
```

```ini
# Hyprland example
exec-once = clipboard-manager
```

**2. Bind a hotkey** to the trigger script:

```ini
bind = SUPER, V, exec, ~/.local/share/clipboard-manager/trigger.sh
```

The daemon auto-creates `~/.local/share/clipboard-manager/trigger.sh` on first run and configures Hyprland window rules automatically.

## Hyprland Troubleshooting

The app auto-detects your Hyprland version and applies the correct window rules. If the window doesn't float, add manually:

**v0.53+:**
```ini
windowrule = float on, match:class floating-clipboard
```

**v0.52 and older:**
```ini
windowrulev2 = float, class:(floating-clipboard)
windowrulev2 = size 900 600, class:(floating-clipboard)
windowrulev2 = center, class:(floating-clipboard)
```

## Data & Security

- **Storage**: `~/.local/share/clipboard-manager/`
- **History**: `clipboard_history.json` (unencrypted)
- **Images**: `images/` subdirectory

Clipboard content is stored in plain text. Be mindful when copying sensitive data.

## Uninstall

```bash
sudo rm /usr/local/bin/clipboard-manager
rm -rf ~/.local/share/clipboard-manager
```

## Docs

For detailed documentation, visit [deepwiki.com/Grenish/clipboard-manager](https://deepwiki.com/Grenish/clipboard-manager).

## License

[MIT](LICENSE)
