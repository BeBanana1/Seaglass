# Image des jobs CI de Seaglass : Node, pnpm, Rust et les bibliothèques de Tauri
# sous Linux, préinstallés pour ne pas les retélécharger à chaque run.
# Construite sur le serveur (voir ci/README.md), jamais poussée dans un registre.

# Debian bookworm : la liste de paquets ci-dessous y a été validée en CI.
FROM node:24-bookworm

RUN apt-get update \
    && apt-get install -y --no-install-recommends \
        build-essential pkg-config \
        libwebkit2gtk-4.1-dev libgtk-3-dev libsoup-3.0-dev \
        libayatana-appindicator3-dev librsvg2-dev \
    && rm -rf /var/lib/apt/lists/*

# Rust installé pour tous les utilisateurs du conteneur. Le workflow garde
# dtolnay/rust-toolchain : il ne télécharge qu'à la sortie d'une nouvelle stable.
ENV RUSTUP_HOME=/usr/local/rustup \
    CARGO_HOME=/usr/local/cargo \
    PATH=/usr/local/cargo/bin:$PATH
RUN curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs \
    | sh -s -- -y --no-modify-path --profile minimal \
        --default-toolchain stable --component rustfmt,clippy

# Analyse des vulnérabilités des dépendances (#263). Sans lui, le workflow saute l'audit
# Rust avec un avertissement (il ne le compile plus : 1 min 44 s à chaque run).
RUN cargo install cargo-audit --locked \
    && rm -rf /usr/local/cargo/registry

ARG PNPM_VERSION=12.16.1
RUN npm install -g pnpm@${PNPM_VERSION}
