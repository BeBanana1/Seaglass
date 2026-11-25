# Audits

Seaglass implementation notes, read on demand (index in [`CLAUDE.md`](../CLAUDE.md)).

## Dependency advisories that cannot be fixed (2026-11-22)

- **`glib` 0.18, RUSTSEC-2024-0429** (unsound `VariantStrIter` iterators, Dependabot asks for `>= 0.20`): pulled in by Tauri itself on Linux (`tauri`, `tauri-runtime-wry`, `wry`, `tao`, `muda`, `webkit2gtk` through `gtk` 0.18, `cargo tree -i glib@0.18.5 --target x86_64-unknown-linux-gnu`). The Rust GTK 3 bindings stopped at 0.18; 0.20 only exists for GTK 4, so no compatible version exists until Tauri moves to GTK 4. Seaglass never calls `VariantStrIter` and the crate is Linux-only. `cargo audit` only warns on it; the Dependabot alert is dismissed as "vulnerable code is not actually used". Recheck when Tauri changes its GTK version.
- Same situation, warnings only: `proc-macro-error` (build-time macro), `zbus_names` / `zvariant` / `zvariant_derive` (D-Bus, Linux), all unmaintained, no fixed upgrade.
- **`chrono` 0.4.45, RUSTSEC-2026-0543** (unmaintained, warning only, seen on 2026-11-25): pulled in by `tauri-utils` through `serde_with`, not by Seaglass. Recheck when Tauri drops it.

## Audit of 2026-11-25 (0.5.117)

Audit by reading the code added since 0.4.94 (remote, playback handoff and its relay tunnel, Pushkin, play log, playlist import, relay) and the Windows `unsafe` code, with `fmt`, `clippy`, the tests, `pnpm check`, `pnpm audit` and `cargo audit` (app and relay: no vulnerability). Checked and sound: one `expect` outside tests (startup), `SAFETY` notes on every Windows `unsafe` block, no token in logs (URLs redacted, Pushkin's signed URL kept out of errors), strict CSPs and no `innerHTML` in the guest and remote pages, tunnel route allowlist (tested), tunnel AEAD bound to direction and request with replay refusal, constant-time code comparison, short links of other services only followed to a recognized link, no secret committed.

Fixed in the relay (to redeploy): bodies were buffered before the rate limit and the lookup of their Jam or device, per-address limits counted each IPv6 address on its own, and PC connections were read at any time with 12 MiB messages (details in `docs/jam.md`, "Relay"). Left open: the remote and handoff server speaks plain HTTP on the local network with a long-lived owner code, which since #430 also unlocked the tunnel route (#62, 0.5.118: the route is only handed out during a pairing window, with a new key each time); the owner code and tunnel key stay in SQLite rather than the credential manager (#61); the local server's read timeout (#60); files well over 400 lines (`Settings.svelte`, `ipc.ts`, `jam.js`, `NowPlaying.svelte`, `App.svelte`, #45). Not covered: on-device testing, the Android Kotlin code, `site/`, ListenBrainz and playlist import in detail, the deployed relay.

## Audit of 2026-11-10 (0.4.94)

Audit of the whole project by reading the code, with `fmt`, `clippy`, the tests and `pnpm check`. Nothing serious. Follow-ups in issues #253 to #265: Jam relay reconnection (#253), Android media session commands (#254), frozen or crashed mpv (#255), Jam limits (#256, #257), sign-in page cookies (#258), session rejected midway (#259), saving on close (#260), mpv from `PATH` (#261), small defects (#262), `cargo audit` in CI (#263), frontend tests (#264), this split of the documentation (#265). Checked on that occasion in the Tauri 2.12.5 code: a remote origin (sign-in window) always goes through the ACL, even for the app's own commands. Not covered: `cargo audit` (tool missing), on-device testing, Windows `unsafe` code in detail.

Small defects (#262, 0.4.97): unreadable fragment ignored by the guest page; `https_only(true)` on the TIDAL, other-service links and APK clients (the `tidal/mod.rs` test that targets `127.0.0.1` over HTTP has its own client); `strip_decorations` looks for "feat." with `to_ascii_lowercase`. Android's `FileProvider` is **kept**: wry (`RustWebChromeClient.createImageFile`) uses it for a photo taken from an `<input type="file">`; its paths are narrowed to `getExternalFilesDir(Pictures)` (`res/xml/file_paths.xml`).

## Audit (0.4.57, #144 to #148)

Security and performance audit of 2026-11-03, by reading the code. Nothing serious on the security side (strict CSP, `core:default` capability, mobile plugin with no command for the UI, manifest server token, Keystore). Fixed: signed URLs in network errors (#144), Android sign-in page and backup (#145), Android rendering (#146), position and saving in the background (#147), token refresh and resolution (#148). Ruled out, with the reason in the issue: filtering media session controllers (#145, risk for the Buds and the lock screen), HTTP compression (#148, TIDAL does not compress). Not handled: virtualization of long lists on Android (to be measured on the S21 first).
