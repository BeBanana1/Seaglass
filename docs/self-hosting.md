# Self-hosting Seaglass

How to build, sign and distribute your own Seaglass, with your own update folder and, if you want them, your own Jam relay and Discord application. Nothing in the code or the scripts depends on the author's server: every address and key is read at build time.

This guide is the path from start to finish. Each step links to the file that documents it in detail, which stays the reference: if the two disagree, trust the linked file (and open an issue).

The release scripts are PowerShell and run on **Windows**; the Linux AppImage is built in WSL from the same PC.

## 1. Prerequisites

- **Build tools**: "Requirements (development)" in the [README](../README.md) for Windows (Rust, Visual Studio Build Tools, Node.js, pnpm, mpv). Android: SDK, NDK and JDK, see [`android.md`](android.md). Linux: a WSL distribution matching the oldest glibc you support (Ubuntu 22.04 by default), see [`linux.md`](linux.md), "Release".
- **mpv for the installer**: the Windows release bundles the mpv found in `PATH` (or `SEAGLASS_MPV_PATH`) only if it matches [`scripts/mpv.lock`](../scripts/mpv.lock): [`release.md`](release.md), "Bundled mpv".
- **TIDAL client identifiers**: "TIDAL client identifiers" in the [README](../README.md). Read what they imply before distributing a build: they are TIDAL's, not this project's, and they end up inside your binary.

Start by checking that a development build works (`pnpm install`, `pnpm tauri dev`, sign in, play a track) before setting up anything below.

## 2. The `.env` file

Copy [`.env.example`](../.env.example) to `.env` at the root (gitignored). It has two kinds of values:

- **Built into the app** by `src-tauri/build.rs`: `TIDAL_CLIENT_ID` / `TIDAL_CLIENT_SECRET`, `SEAGLASS_UPDATE_BASE_URL`, `SEAGLASS_UPDATER_PUBKEY`, `SEAGLASS_RELAY_BASE_URL` / `SEAGLASS_RELAY_KEY`, `DISCORD_CLIENT_ID`. Changing one means a new build: installed copies keep what they were built with.
- **Read by the release scripts only**, never built in: `SEAGLASS_RELEASE_DEST`, `SEAGLASS_SITE_DEST`, `SEAGLASS_GITHUB_*`, the Android key settings. The bottom of `.env.example` lists them.

An empty value turns the feature off: no update address = no update offered, no relay = local Jam only, no Discord identifier = setting hidden.

## 3. Signing keys

Two keys, both **outside the repository**, both to **back up**: an installed copy only accepts updates signed by the key it was built with. Losing a key means every user has to uninstall and reinstall by hand.

- **Updater key** (Windows installer and Linux AppImage):

  ```powershell
  pnpm tauri signer generate -w $HOME\.tauri\seaglass.key
  ```

  The scripts read the private key from `SEAGLASS_SIGNING_KEY` (default `~/.tauri/seaglass.key`) and build in the public key from the `.pub` next to it (or `SEAGLASS_UPDATER_PUBKEY`). Password handling: [`release.md`](release.md), "Signing" and "Key password".
- **Android keystore**: created once with `keytool`, its password in the environment, the Credential Manager or typed at each build: [`android.md`](android.md), "Distribution".

## 4. Update hosting

You need a **public folder served over HTTPS**, without authentication (the app reads it anonymously; what protects updates is the signature, not access to the folder: [`release.md`](release.md), "No authentication on that folder").

- `SEAGLASS_UPDATE_BASE_URL`: the public address of that folder, for example `https://updates.example.org/seaglass` (no trailing `/`). Built into the app.
- `SEAGLASS_RELEASE_DEST`: where the scripts copy to, the same folder seen from your PC: a local or UNC path, or an scp target (`user@host:/srv/updates/seaglass` or an `~/.ssh/config` alias). A deploy user with only write access to that folder is enough.

The scripts upload, for each version: `<version>/` with the installer, the APK, the AppImage and their signatures, then the manifests `android.json`, `linux.json`, `windows.json` and, last, `latest.json` (the desktop updater's manifest). A client never sees a manifest before the file it points to.

Minimal Caddy block (any static HTTPS server works):

```caddyfile
updates.example.org {
    handle_path /seaglass/* {
        root * /srv/updates/seaglass
        header /*.json Cache-Control "no-cache"
        file_server
    }
}
```

`no-cache` on the manifests makes clients see a new version at once. The author's full configuration, with security headers and the website on the same domain: [`release.md`](release.md), "Caddy block".

**Website** (optional): the download page reads the same manifests. `scripts/publish-site.ps1` publishes `site/` to `SEAGLASS_SITE_DEST`: [`release.md`](release.md), "Website". It expects the manifests under `/updates/` of its own domain; with another layout, adapt the paths in `site/`. It also builds the app demo served at `/try/` from your latest `v*` tag; that folder needs its own CSP (same section).

## 5. Releasing

1. Bump `VERSION` and `src-tauri/Cargo.toml`, add the `CHANGELOG.md` entry, commit: "Publishing a version" in the [README](../README.md).
2. Build everything without publishing:

   ```powershell
   pnpm release:all
   ```

   Windows installer, APK, then AppImage in WSL (`-SkipLinux` to leave it out). A failed build publishes nothing.
3. Publish:

   ```powershell
   pnpm release:all -Publish
   ```

   Uploads to `SEAGLASS_RELEASE_DEST` (Windows last, since its `latest.json` also announces the AppImage), sets and pushes the `v<version>` tag, then creates a GitHub release with the same files. For that release: `SEAGLASS_GITHUB_REPO` (`owner/name`) and a fine-grained token of that repository with "Contents: Read and write" (`SEAGLASS_GITHUB_TOKEN` or the Credential Manager entry `seaglass-github-token`). Without them, add `-SkipGitHubRelease`.

Every option (`-PublishOnly`, `-Notes`, per-platform scripts): [`release.md`](release.md), "Full release" and "GitHub release".

## 6. Optional services

- **Jam relay** (online Jam and playback handoff between the computer and the phone): a small Rust server in Docker behind Caddy. Generate a key (`openssl rand -hex 32`), put the same value in `relay/.env` and in the app's `.env` (`SEAGLASS_RELAY_KEY`), with `SEAGLASS_RELAY_BASE_URL` = the relay's public address. Deployment step by step: [`relay/README.md`](../relay/README.md). Users can also pick another relay, or none, in Settings › Application › Relay.
- **Discord presence**: create an application in the Discord Developer Portal and put its numeric Application ID in `DISCORD_CLIENT_ID`. Its name is what Discord shows ("Listening to …"). Details: [`system.md`](system.md), "Discord presence".

## 7. Checks

- **Updates**: install a build of version N from your folder, publish N+1, then start the installed copy. About 20 seconds later it offers "Install and restart" (desktop) or the new APK (Android). Nothing offered: check that `<SEAGLASS_UPDATE_BASE_URL>/latest.json` (or `android.json`) opens in a browser and names the new version, and that the installed build was made with your `.env` and your key (a build signed by another key is never offered).
- **Manifests online**: `curl -sI <SEAGLASS_UPDATE_BASE_URL>/latest.json` answers `200`.
- **Relay**: `curl https://<relay>/healthz` answers `ok`; then start an online Jam in the app (not "On the local network") and open its link from a phone on mobile data.
- **Discord**: with Discord open on the same computer, play a track: the presence appears after a few seconds.
