# Linux (#17)

Status: the app builds and plays on Linux, with no published package and no updates. mpv comes from the
system (`apt install mpv`), it is not bundled.

## What differs from the Windows desktop

| Area | Windows | Linux |
|---|---|---|
| Audio output | `--ao=wasapi` | `--ao=pipewire,pulse,alsa`: mpv takes the first one that opens (`mpv::device::AO`) |
| Device list | `wasapi/` entries | entries of the first driver present among `pipewire/`, `pulse/`, `alsa/` (`device::pick`): otherwise PipeWire shows up again through its PulseAudio layer and through ALSA |
| Default device name | `IMMDeviceEnumerator` | unknown (`system_default_id` returns `None`): the signal path shows "system default" |
| Exclusive mode (#245) | `--audio-exclusive` | not offered: neither PipeWire nor PulseAudio provide it through mpv, the switch is hidden (`exclusive_available`) |
| mpv | bundled; `PATH` only in debug (#261) | the system's (`PATH`, in release too: `execvp` does not search the current folder) |
| Stopping mpv on a crash | Job Object (`mpv/job.rs`) | `prctl(PR_SET_PDEATHSIG, SIGKILL)` (`process::die_with_parent`) |
| mpv IPC | named pipe | Unix socket in the temporary folder |
| Sign-in | WebView2 window, redirect intercepted | system browser, redirect address pasted into the app (see below) |
| Media controls | SMTC | nothing yet (MPRIS to do, separate issue) |
| Tokens | Credential Manager (`windows-native`) | Secret Service (`sync-secret-service` + `crypto-rust`, a Linux-target-only dependency in `Cargo.toml`): needs a keyring (GNOME Keyring, KWallet). Without a backend, `keyring` 3 falls back to an **in-memory** store and the session is lost on close: that was the case before (#17). With no service available, sign-in fails with an error instead of silently losing the session. `secret-service` and `zbus` 4 appear in `Cargo.lock` through an optional `keyring` feature, without being compiled |

Bit-perfect: PipeWire follows the stream's sample rate if `default.clock.allowed-rates` contains
44.1 and 48 kHz (and their multiples) in its configuration; otherwise it resamples. Direct
ALSA (`alsa/hw:…`) would be the equivalent of exclusive mode: to be investigated.

## Sign-in through the system browser

TIDAL serves a bot check to WebKitGTK and, after a few sign-ins,
**blocks** it ("You have been blocked", seen on 2026-11-11 in WSL). On Linux, sign-in therefore
goes through the user's browser, like `tidalapi`'s PKCE flow:

1. `browser_login_start` creates the PKCE challenge, keeps it in `commands::auth::BROWSER_LOGIN`
   (one at a time), opens `login.tidal.com/authorize` through `tauri-plugin-opener` and
   returns the address. If no browser opens (WSL has none, `xdg-open` fails),
   the failure is only logged: the screen shows the address with a Copy button, to
   paste into any browser, including the Windows one. It only contains the
   challenge, not the verifier.
2. After sign-in, TIDAL redirects to `https://tidal.com/android/login/auth?code=…`, an
   error page in a desktop browser. The user pastes its address into the app.
3. `browser_login_finish` only accepts that address (`auth::redirect_code`, 4 KB at most,
   tested). The challenge is only removed once the session is open: otherwise a failure of
   the exchange or of the keyring, followed by a new attempt, only showed "open the sign-in
   page first" and hid the real error (seen in WSL).

The choice comes from Rust (`login_in_browser`, true on Linux); the screen is
`views/Login.svelte`. Preview: `?loggedout&browserLogin=1`.

## Building and testing in WSL

WSLg provides the display (Wayland/X) and a PulseAudio server (`PULSE_SERVER`) that outputs to
the Windows default device. Nothing there is bit-perfect (resampled RDP chain):
WSL is for checking that the app starts, signs in and plays, not the quality.

Packages (Ubuntu 24.04, as root: `wsl -u root`):

```bash
apt-get install -y pkg-config libwebkit2gtk-4.1-dev libgtk-3-dev libayatana-appindicator3-dev librsvg2-dev libdbus-1-dev libxdo-dev file patchelf mpv gnome-keyring libsecret-tools
```

Node 20+ (Vite 8) is not in the Ubuntu 24.04 repositories: the frontend is built on
Windows (`pnpm build`), then the tree (with `dist/` and `.env`) is copied into the WSL home
(building on `/mnt/g` is very slow). `tauri-cli` comes from crates.io
(`cargo install tauri-cli --version "^2" --locked`). `beforeBuildCommand` is neutralized, and
so are the update artifacts (they require the signing key):

```bash
cd ~/seaglass/src-tauri && cargo tauri build --bundles appimage --config '{"build":{"beforeBuildCommand":""},"bundle":{"createUpdaterArtifacts":false}}'
```

**GStreamer in the AppImage** (`bundle.linux.appimage.bundleMediaFramework`): without it,
the AppImage bundles `libgstreamer` but no plugin, and does not find the system's
(`appsink`, `appsrc`, `autoaudiosink` "not found"). TIDAL's bot check page,
served to WebKitGTK at sign-in, then did not show its challenge and
sign-in was impossible. The bare binary (`target/release/seaglass`), which uses the
system GStreamer, showed it. With linuxdeploy's `gstreamer` plugin (it needs
`patchelf`), the AppImage goes from 83 to 97 MB.

The AppImage is launched with `--appimage-extract-and-run` (no FUSE in WSL). The
`wsl.exe` session that launched it must stay open: the app stops with it.

## Release (#313)

The published AppImage is built on **Ubuntu 22.04**, in a dedicated WSL distribution
(`Ubuntu-22.04`), not in the test one (24.04). An AppImage requires the glibc of the
machine that built it: on 24.04, the binary required `GLIBC_2.39` and did not start
on Debian 12 (2.36), Ubuntu 22.04 (2.35) or Mint 21. 22.04 is the oldest base
that has WebKitGTK 4.1 (required by Tauri 2): glibc 2.35, i.e. Ubuntu 22.04,
Debian 12, Mint 21, Fedora 36 and newer (stated on the download page).

- **Preparing the distribution** (once, rerunnable): `wsl --install -d Ubuntu-22.04
  --no-launch`, a user created as root (`useradd -m`, `[user] default=` in
  `/etc/wsl.conf`), then `scripts/linux-build-setup.sh --system` as root (packages, GStreamer
  plugins bundled by `bundleMediaFramework`, mpv and playerctl for testing) and
  `scripts/linux-build-setup.sh` as the user (rustup, `tauri-cli`).
- **Build**: `powershell -File scripts/release-linux.ps1` (called by `release-all.ps1`,
  which accepts `-SkipLinux`). `pnpm build` on Windows, then `scripts/linux-build.sh` in
  the distribution: copies only the repository's files into `~/seaglass-release`
  (`git ls-files`, plus `.env` and `dist/`, list in `release/.build/linux-files.txt`),
  after emptying the previous copy except `src-tauri/target` (#418). Reading `/mnt/g` is
  expensive per file: the old `rsync` of the whole tree took `.claude/worktrees` along (88,000
  files, 62 GB) and `relay/target`, and it was the slowest step of the build; copying ~560
  files takes about 8 s. Then
  `cargo tauri build --bundles appimage`, signed for the updater (#314, key through `WSLENV`), then
  a **glibc check**: the highest `GLIBC_x.y` required by the binary and by the `.so` files
  of the `AppDir` must be ≤ `SEAGLASS_LINUX_GLIBC_MAX` (2.35 by default), otherwise the build
  fails. Outputs: `release/<version>/Seaglass_<version>_amd64.AppImage` and its `.sig`,
  `release/linux.json` (version, URL, SHA-256, size, glibc, notes) and the marker
  `release/.build/linux.json`. `-PublishOnly`, publishing (AppImage then manifest), tag
  and GitHub release as for Android.
- **Download page**: Linux card filled from `linux.json` (same code as the
  Android card), with the note "mpv required".
- First build (2026-11-11, 0.5.7): full build in 2 min, 94 MB AppImage,
  glibc 2.35 required, 106 GStreamer plugins bundled. Launched on Ubuntu 24.04: queue
  resumed and HiRes 24/96 playback, MPRIS published. Launched on 22.04: the app and mpv
  start; the build distribution has no session bus (its systemd user session
  does not start), so MPRIS is missing there, with a simple warning.
- **mpv 0.34** (Ubuntu 22.04, the oldest version targeted; Debian 12 has 0.35):
  `loadfile` with named arguments (#316) accepted, with `start` and `af` applied;
  `seaglass.conf` only gives the two already known warnings (`media-controls`,
  `wasapi-exclusive-buffer`).

## Updates (#314)

How it works and the manifest: `release.md` ("Updates" and "Updater manifest"
bullets). End-to-end test on 2026-11-11 (0.5.8), with a **throwaway test key**
and a manifest served at `http://127.0.0.1:8765` (`dangerousInsecureTransportProtocol` in
the test build's config only), two AppImages built on 22.04 (0.5.8 and a
fake 0.5.9 through `"version"` in `--config`), launched on 24.04 as a **real AppImage**
(FUSE: `libfuse2t64`, `/dev/fuse` exists in WSL2):

- AppImage in `~/Apps`: manifest read, 0.5.9 downloaded, signature verified,
  "Update ready" offered; "Install and restart": file replaced by
  0.5.9 (checksum), app relaunched by `request_restart`, the new instance no longer offers
  anything.
- AppImage in a `555` folder: message "cannot replace the AppImage: the
  folder … is not writable …", file intact.
- `--appimage-extract-and-run`: Tauri validates `APPIMAGE`, the updater is active and
  would target the real file.
- Bare binary (`--no-bundle`): "automatic updates disabled".
- Driving the window in WSLg: `GDK_BACKEND=x11`, then `xwininfo` (absolute position),
  `xdotool` (click) and `import` (capture); `wmctrl` does not see the windows.

During these tests, WSLg's `pulse` output froze (mpv stuck at 0 % with or without
the app): playback could not be rechecked on the updated AppImage.

## mpv environment under AppImage (#320)

The `AppRun` sets variables that point into the mount point (`LD_LIBRARY_PATH`,
`GST_PLUGIN_*`, `GIO_MODULE_DIR`, `GTK_*`, `GDK_PIXBUF_MODULE_FILE`, `PYTHONHOME`,
`XDG_DATA_DIRS`, `PATH`…: 21 variables); after a relaunch, the environment also keeps
those of the old mount. mpv, a system binary, inherited them: with
`LD_LIBRARY_PATH` alone, `ldd /usr/bin/mpv` resolved 95 libraries inside the AppImage (those
of Ubuntu 22.04). `mpv/process.rs` (`clean_appimage_env`, if `APPDIR` is set) applies
the corrections of `mpv/appimage_env.rs` (pure tested logic) to mpv's command: path
lists lose their entries located under `APPDIR` or under a `.mount_*`
folder, and empty entries; a variable that only pointed there is removed. The app
keeps its environment. Verified on 2026-11-11 (0.5.9, real AppImage on 24.04): 21
variables pointing to the AppImage in the app, 0 in `/proc/<mpv>/environ`, no AppImage
library in `/proc/<mpv>/maps`.

## Verified (2026-11-11, WSL Ubuntu 24.04, mpv 0.37)

- AppImage 0.5.5 built (83 MB); the app starts, system mpv launched as a child.
- `seaglass.conf`: options missing from mpv 0.37 or specific to Windows
  (`media-controls`, `wasapi-exclusive-buffer`) only give a warning, mpv starts.
- `audio-device-list` in WSLg: `pulse/RDPSink`, `alsa`, `jack`, `sdl` → `device::pick`
  keeps `pulse/RDPSink`. With `--ao=pipewire,pulse,alsa`, mpv tries `pipewire` then opens
  `pulse` (an `audio-device` prefixed with `pulse/` does not make it skip `pipewire`).
- `kill -9` of the app: mpv disappears at once (`PR_SET_PDEATHSIG`).
- Sign-in in WebKitGTK (bare binary, then AppImage): TIDAL serves a bot
  check; once the user solves the challenge, the redirect is intercepted
  and the session opened. After a few sign-ins, TIDAL blocked WebKitGTK:
  hence sign-in through the system browser.

- Sign-in from the AppImage with bundled GStreamer: challenge shown, session opened.
  Without a Linux backend for `keyring`, the session was lost on close.

WSL has a D-Bus session bus (systemd user): `gnome-keyring` is activated there on
demand, but **without a default keyring** (the `default` alias points to a nonexistent
`login`), and `keyring` does not create it: documented limitation in
`keyring/src/secret_service.rs` (keyring-rs#133). Create it once with `secret-tool`, which
opens the GNOME window to choose its password, then clear the test entry:

```bash
secret-tool store --label=test seaglass-test x
secret-tool clear seaglass-test x
```

A real desktop (GNOME, KDE) has this keyring as soon as the session opens.

- Sign-in through the system browser (address copied into the Windows browser,
  TIDAL's final error page pasted into the app): session opened, stored in
  GNOME Keyring once the keyring was created. TIDAL playback through mpv on the `pulse` output.

- Session kept after restarting the app, gapless playback between the tracks
  of an album, queue and current track resumed paused on restart.

- `loadfile` (#316): the positional form `loadfile <url> <flags> -1 <options>` only exists
  since mpv 0.38 (`index` argument added before `options`). Ubuntu 24.04's mpv 0.37
  answered `invalid parameter` to any resume at a position and to normalization. The
  engine now sends `loadfile` with **named arguments**
  (`{"name": "loadfile", "url", "flags", "options"}`), understood by 0.37 as well as 0.41:
  `live_per_file_af` passes in both cases, and resuming the queue at 91 s plays.
- Media controls (MPRIS, #312) driven by `playerctl`: see `system.md`. To try it
  without an AppImage: `cargo tauri build --no-bundle` with the same configuration, then
  `target/release/seaglass` (same data folder, same keyring).

Not verified: notification area (WSLg has no representative one), media applet and
media keys of a real desktop.
