# Clipboard Manager

A lightweight, terminal-based clipboard manager for Linux, designed for efficiency and seamless integration with tiling window managers like Hyprland.

[![License](https://img.shields.io/badge/license-MIT-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/language-Rust-orange.svg)](https://www.rust-lang.org/)

## Overview

Clipboard Manager is a high-performance daemon and TUI (Text User Interface) tool that captures and manages clipboard history. It supports both text and images, offering a persisting history that survives reboots. Built with Rust, it prioritizes low resource usage and speed, making it an ideal companion for keyboard-centric workflows on Wayland and X11.

## Features

- **Clipboard History**: Automatically captures the last 50 clipboard entries.
- **Content Support**: Handles both text and images (with preview metadata).
- **Persistence**: History is saved to disk, preserving context across sessions.
- **Daemon Mode**: Efficient background process for continuous monitoring.
- **Terminal UI**: Fast, responsive `ratatui`-based interface for history navigation.
- **Protocol Support**: Native support for Wayland (`wl-clipboard`) and X11.
- **Deduplication**: Content hashing prevents duplicate entries.

## Architecture

The system operates as two distinct components:

1.  **Daemon**: A background service that polls the system clipboard (150ms interval), detects changes, computes content hashes, and updates the persistent storage.
2.  **TUI Client**: A transient interface triggered by the user to browse history and inject selected content back into the system clipboard.

![Architecture Diagram](dia.svg)

## Installation

### AUR (Arch User Repository)

For Arch Linux users, the package is available via AUR:

```bash
yay -S clipboard-manager-rs-git
```

### From Source

Ensure you have Rust and Cargo installed.

```bash
git clone https://github.com/grenish/clipboard-manager.git
cd clipboard-manager
cargo install --path .
```

*Note: Wayland users require `wl-clipboard` installed.*

## Usage

### 1. Start the Daemon

Launch the daemon in the background (preferably via your window manager's init script):

```bash
clipboard-manager &
```

### 2. Configure the Trigger

The daemon generates a trigger script at `~/.local/share/clipboard-manager/trigger.sh`. Bind this to a hotkey.

**Hyprland Configuration Example:**

```ini
# Trigger with Super+Comma
bind = SUPER, comma, exec, ~/.local/share/clipboard-manager/trigger.sh

# Float and center the UI
windowrulev2 = float, class:(floating-clipboard)
windowrulev2 = size 900 600, class:(floating-clipboard)
windowrulev2 = center, class:(floating-clipboard)
windowrulev2 = animation popin, class:(floating-clipboard)
windowrulev2 = stayfocused, class:(floating-clipboard)
```

## Data Storage & Security

- **Storage Location**: `~/.local/share/clipboard-manager/`
- **History File**: `clipboard_history.json` (Plain text)
- **Images**: `images/` subdirectory

**Security Notice**: Clipboard content is stored unencrypted. Avoid copying sensitive secrets (passwords, keys) if disk encryption is not active, or be mindful to clear the history.

## License

This project is open-source and licensed under the [MIT License](LICENSE).
