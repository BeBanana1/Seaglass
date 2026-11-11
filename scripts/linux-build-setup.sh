#!/bin/bash
# Prépare la distribution WSL de build Linux (Ubuntu 22.04, #313). Relançable sans risque.
#
# Ubuntu 22.04 : la plus ancienne base qui a WebKitGTK 4.1 (exigé par Tauri 2). Sa glibc
# (2.35) fixe le minimum de l'AppImage : Ubuntu 22.04, Debian 12, Mint 21, Fedora 36 et
# plus récentes. Construite sous 24.04, l'AppImage exigeait la glibc 2.39.
#
# En root (paquets du système) :
#   wsl -d Ubuntu-22.04 -u root bash scripts/linux-build-setup.sh --system
# Puis en utilisateur (Rust et tauri-cli) :
#   wsl -d Ubuntu-22.04 bash scripts/linux-build-setup.sh
set -euo pipefail

if [ "${1:-}" = "--system" ]; then
    export DEBIAN_FRONTEND=noninteractive
    apt-get update
    # Build : en-têtes de Tauri et de WebKitGTK, linuxdeploy (file, patchelf), et les
    # plugins GStreamer que `bundleMediaFramework` embarque dans l'AppImage.
    # Essai : mpv, trousseau et playerctl, pour lancer l'app construite.
    apt-get install -y --no-install-recommends \
        build-essential pkg-config curl ca-certificates rsync file patchelf \
        libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev \
        libdbus-1-dev libxdo-dev libssl-dev \
        gstreamer1.0-plugins-base gstreamer1.0-plugins-good gstreamer1.0-gl gstreamer1.0-x \
        mpv gnome-keyring libsecret-tools playerctl dbus-user-session
    exit 0
fi

if [ "$(id -u)" = "0" ]; then
    echo "Rust s'installe pour l'utilisateur de build, pas en root (voir l'en-tête)." >&2
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
