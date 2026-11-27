#!/bin/bash
# Sets up the WSL distribution used for Linux builds (Ubuntu 22.04, #313). Safe to rerun.
#
# Ubuntu 22.04: the oldest base that has WebKitGTK 4.1 (required by Tauri 2). Its glibc
# (2.35) sets the AppImage minimum: Ubuntu 22.04, Debian 12, Mint 21, Fedora 36 and
# newer. Built on 24.04, the AppImage required glibc 2.39.
#
# As root (system packages):
#   wsl -d Ubuntu-22.04 -u root bash scripts/linux-build-setup.sh --system
# Then as a user (Rust and tauri-cli):
#   wsl -d Ubuntu-22.04 bash scripts/linux-build-setup.sh
set -euo pipefail

if [ "${1:-}" = "--system" ]; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    # Build: Tauri and WebKitGTK headers, linuxdeploy (file, patchelf), and the GStreamer
    # plugins that `bundleMediaFramework` embeds in the AppImage.
    # Trial: mpv, keyring and playerctl, to run the built app.
    apt-get install -y --no-install-recommends \
        build-essential pkg-config curl ca-certificates rsync file patchelf \
        libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
        libdbus-1-dev libxdo-dev libssl-dev \
        gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-gl gstreamer1.0-x \
        mpv gnome-keyring libsecret-tools playerctl dbus-user-session
    exit 0
fi

if [ "$(id -u)" = "0" ]; then
    echo "Rust is installed for the build user, not as root (see the header)." >&2
    exit 1
fi

if ! command -v rustup >/dev/null && [ ! -x "$HOME/.cargo/bin/rustup" ]; then
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
fi
# shellcheck disable=SC1091
source "$HOME/.cargo/env"
rustup update stable
if ! cargo tauri --version >/dev/null 2>&1; then
    cargo install tauri-cli --version "^2" --locked
fi
rustc --version
cargo tauri --version
