# Clipboard Manager

A lightweight, terminal-based clipboard manager for Linux built with Rust. Designed for tiling WMs like Hyprland.

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)
[![Docs](https://img.shields.io/badge/docs-deepwiki-purple.svg)](https://deepwiki.com/Grenish/clipboard-manager)

## Features

- **Clipboard history** — last 50 entries, text & images
- **Smart deduplication** — re-copied content moves to top
- **Persistent history** across reboots
- **Pinning** — pin important entries so they always appear at the top and are never evicted
- **Smart content detection** — automatically categorizes entries as 🔗 Link, 📧 Email, 🎨 Color, 📁 Path, 📞 Phone, 💻 Code, or 📝 Text
- **Sensitive content detection** — detects API keys, tokens, private keys, JWTs, and credit card numbers; masks them by default with auto-expiry (5 min)
- **Emoji/emoticon picker** — browse 8 categories in a grid layout, search by name, and paste with Enter
- **Auto-detection** of Hyprland with floating window rules
- **Background daemon** + `ratatui` TUI
- **Wayland** (`wl-clipboard`) and **X11** support

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

## Keybindings

### Main View

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate entries |
| `Enter` | Copy & paste selected entry |
| `/` | Search clipboard history |
| `P` | Toggle pin on selected entry |
| `R` | Reveal / hide a masked secret |
| `⇧S` | Stop auto-expiry on a secret (make permanent) |
| `E` | Open emoji picker |
| `C` | Clear all history (with confirmation) |
| `Esc` / `q` | Quit |

### Emoji Picker

| Key | Action |
|-----|--------|
| `↑` / `↓` | Navigate grid rows |
| `←` / `→` | Navigate grid cells |
| `Tab` / `⇧Tab` | Switch category |
| `Enter` | Copy & paste selected emoticon |
| Type | Search emoticons by name |
| `Esc` | Close picker / clear search |

### Search

Searching matches against content text **and** category labels — type `code`, `email`, `link`, `secret`, etc. to filter by detected type.

## Smart Detection

Entries are automatically categorized at display time with no extra storage:

| Icon | Category | Examples |
|------|----------|----------|
| 🔗 | Link | URLs starting with `http://`, `https://`, `ftp://` |
| 📧 | Email | Addresses matching `user@domain.tld` |
| 🎨 | Color | Hex codes like `#ff5733`, `rgb(...)`, `hsl(...)` |
| 📁 | Path | Unix paths like `/home/user/file.txt`, `~/docs` |
| 📞 | Phone | Phone numbers like `+1-555-123-4567` |
| 💻 | Code | Multi-line text with code indicators (`{`, `fn`, `def`, etc.) |
| 📝 | Text | Everything else |

## Sensitive Content Protection

The clipboard manager detects sensitive content and protects it automatically:

- **Detected providers**: OpenAI, GitHub, AWS, Slack, Stripe, Google/Gemini, and more
- **Detected patterns**: API key prefixes, private key blocks, JWTs, Bearer tokens, credit card numbers (Luhn check), high-entropy secrets
- **Auto-masking**: secrets are displayed as `••••••••` by default
- **Auto-expiry**: secrets are automatically deleted after 5 minutes
- **Controls**: press `R` to reveal, `⇧S` to stop expiry

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
- **History**: `clipboard_history.jsonl` (JSONL format, unencrypted)
- **Images**: `images/` subdirectory
- **Secrets**: auto-expire after 5 minutes; masked in the TUI by default

Clipboard content is stored in plain text. Be mindful when copying sensitive data. Detected secrets are automatically cleaned up after expiry.

## Uninstall

```bash
sudo rm /usr/local/bin/clipboard-manager
rm -rf ~/.local/share/clipboard-manager
```

## Docs

For detailed documentation, visit [deepwiki.com/Grenish/clipboard-manager](https://deepwiki.com/Grenish/clipboard-manager).

## License

[MIT](LICENSE)