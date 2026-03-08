#!/usr/bin/env bash
#
# Clipboard Manager Installer
# https://github.com/Grenish/clipboard-manager
#
# Usage:
#   curl -fsSL https://raw.githubusercontent.com/Grenish/clipboard-manager/main/install.sh | bash
#
# Environment variables:
#   INSTALL_DIR    — Override install directory (default: /usr/local/bin)
#   VERSION        — Install a specific version tag (default: latest)
#

set -euo pipefail

# ── Branding ──────────────────────────────────────────────────────────────────

REPO="Grenish/clipboard-manager"
BIN_NAME="clipboard-manager"
GITHUB_API="https://api.github.com"
GITHUB_RELEASES="https://github.com/${REPO}/releases"

# ── Colors ────────────────────────────────────────────────────────────────────

if [ -t 1 ]; then
    RED='\033[0;31m'
    GREEN='\033[0;32m'
    YELLOW='\033[1;33m'
    CYAN='\033[0;36m'
    BOLD='\033[1m'
    RESET='\033[0m'
else
    RED='' GREEN='' YELLOW='' CYAN='' BOLD='' RESET=''
fi

info()  { printf "${CYAN}info${RESET}  %s\n" "$*"; }
warn()  { printf "${YELLOW}warn${RESET}  %s\n" "$*"; }
error() { printf "${RED}error${RESET} %s\n" "$*" >&2; }
ok()    { printf "${GREEN}  ok${RESET}  %s\n" "$*"; }

# ── Helpers ───────────────────────────────────────────────────────────────────

check_architecture() {
    local arch
    arch="$(uname -m)"
    case "$arch" in
        x86_64|amd64)  echo "x86_64" ;;
        aarch64|arm64) echo "aarch64" ;;
        *)
            error "Unsupported architecture: $arch"
            error "Only x86_64 and aarch64 are supported."
            exit 1
            ;;
    esac
}

check_os() {
    local os
    os="$(uname -s)"
    case "$os" in
        Linux) echo "linux" ;;
        *)
            error "Unsupported operating system: $os"
            error "Clipboard Manager only supports Linux."
            exit 1
            ;;
    esac
}

get_latest_version() {
    local url="${GITHUB_API}/repos/${REPO}/releases/latest"
    local version

    if command -v curl &>/dev/null; then
        version="$(curl -fsSL "$url" | grep '"tag_name"' | head -1 | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/')"
    elif command -v wget &>/dev/null; then
        version="$(wget -qO- "$url" | grep '"tag_name"' | head -1 | sed -E 's/.*"tag_name":\s*"([^"]+)".*/\1/')"
    else
        error "Either 'curl' or 'wget' is required."
        exit 1
    fi

    if [ -z "$version" ]; then
        error "Failed to fetch latest version from GitHub."
        error "Please check your internet connection or specify VERSION manually."
        error "  VERSION=v0.1.6 bash install.sh"
        exit 1
    fi

    echo "$version"
}

download() {
    local url="$1"
    local output="$2"

    if command -v curl &>/dev/null; then
        curl -fSL --progress-bar -o "$output" "$url"
    elif command -v wget &>/dev/null; then
        wget -q --show-progress -O "$output" "$url"
    else
        error "Either 'curl' or 'wget' is required."
        exit 1
    fi
}

# ── Dependency Check ──────────────────────────────────────────────────────────

check_runtime_deps() {
    local missing=()

    if ! command -v wl-copy &>/dev/null || ! command -v wl-paste &>/dev/null; then
        missing+=("wl-clipboard")
    fi
    if ! command -v wtype &>/dev/null; then
        missing+=("wtype")
    fi

    if [ ${#missing[@]} -gt 0 ]; then
        warn "Optional runtime dependencies not found: ${missing[*]}"
        warn "Wayland paste functionality requires: wl-clipboard, wtype"
        echo ""
    fi
}

# ── Main ──────────────────────────────────────────────────────────────────────

main() {
    echo ""
    printf "${BOLD}  Clipboard Manager Installer${RESET}\n"
    printf "  ${CYAN}${GITHUB_RELEASES}${RESET}\n"
    echo ""

    # Preflight
    local os arch version install_dir
    os="$(check_os)"
    arch="$(check_architecture)"

    install_dir="${INSTALL_DIR:-/usr/local/bin}"

    if [ -n "${VERSION:-}" ]; then
        version="$VERSION"
        info "Using specified version: $version"
    else
        info "Fetching latest version..."
        version="$(get_latest_version)"
        ok "Latest version: $version"
    fi

    # The release asset is the raw binary named "clipboard-manager"
    local download_url="${GITHUB_RELEASES}/download/${version}/${BIN_NAME}"

    info "Platform: ${os} ${arch}"
    info "Version:  ${version}"
    info "Binary:   ${BIN_NAME}"
    info "Target:   ${install_dir}/${BIN_NAME}"
    echo ""

    # Check runtime deps (non-fatal)
    check_runtime_deps

    # Create temp directory
    local tmp_dir
    tmp_dir="$(mktemp -d)"
    trap 'rm -rf "$tmp_dir"' EXIT

    # Download the binary directly
    info "Downloading ${BIN_NAME} (${version})..."
    download "$download_url" "${tmp_dir}/${BIN_NAME}"
    ok "Download complete."

    # Make it executable
    chmod 755 "${tmp_dir}/${BIN_NAME}"

    # Verify it's a valid binary
    if ! file "${tmp_dir}/${BIN_NAME}" 2>/dev/null | grep -qi "elf"; then
        error "Downloaded file does not appear to be a valid Linux binary."
        error "Please check the release assets at: ${GITHUB_RELEASES}/tag/${version}"
        exit 1
    fi

    ok "Verified binary."

    # Install
    info "Installing to ${install_dir}/${BIN_NAME}..."

    if [ -w "$install_dir" ]; then
        install -Dm755 "${tmp_dir}/${BIN_NAME}" "${install_dir}/${BIN_NAME}"
    else
        warn "Elevated permissions required to write to ${install_dir}"
        if command -v sudo &>/dev/null; then
            sudo install -Dm755 "${tmp_dir}/${BIN_NAME}" "${install_dir}/${BIN_NAME}"
        elif command -v doas &>/dev/null; then
            doas install -Dm755 "${tmp_dir}/${BIN_NAME}" "${install_dir}/${BIN_NAME}"
        else
            error "Cannot write to ${install_dir} and neither 'sudo' nor 'doas' is available."
            error "Try running as root or set INSTALL_DIR to a writable path:"
            error "  INSTALL_DIR=~/.local/bin bash install.sh"
            exit 1
        fi
    fi

    ok "Installed ${BIN_NAME} to ${install_dir}/${BIN_NAME}"

    # Verify
    if command -v "$BIN_NAME" &>/dev/null; then
        ok "Verified: $(command -v "$BIN_NAME")"
    else
        warn "${BIN_NAME} was installed but is not in your PATH."
        warn "Add the following to your shell profile:"
        echo ""
        echo "    export PATH=\"${install_dir}:\$PATH\""
        echo ""
    fi

    # Done
    echo ""
    printf "${GREEN}${BOLD}  ✓ Installation complete!${RESET}\n"
    echo ""
    echo "  Quick start:"
    echo "    1. Start the daemon:  ${BIN_NAME} &"
    echo "    2. Bind a hotkey to:  ~/.local/share/clipboard-manager/trigger.sh"
    echo ""
    echo "  Example (Hyprland):"
    echo "    bind = SUPER, V, exec, ~/.local/share/clipboard-manager/trigger.sh"
    echo ""
    echo "  To uninstall:"
    echo "    sudo rm ${install_dir}/${BIN_NAME}"
    echo "    rm -rf ~/.local/share/clipboard-manager"
    echo ""
}

main "$@"
