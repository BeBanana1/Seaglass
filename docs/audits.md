# Audits

Seaglass implementation notes, read on demand (index in [`CLAUDE.md`](../CLAUDE.md)).

## Audit of 2026-11-10 (0.4.94)

Audit of the whole project by reading the code, with `fmt`, `clippy`, the tests and `pnpm check`. Nothing serious. Follow-ups in issues #253 to #265: Jam relay reconnection (#253), Android media session commands (#254), frozen or crashed mpv (#255), Jam limits (#256, #257), sign-in page cookies (#258), session rejected midway (#259), saving on close (#260), mpv from `PATH` (#261), small defects (#262), `cargo audit` in CI (#263), frontend tests (#264), this split of the documentation (#265). Checked on that occasion in the Tauri 2.12.5 code: a remote origin (sign-in window) always goes through the ACL, even for the app's own commands. Not covered: `cargo audit` (tool missing), on-device testing, Windows `unsafe` code in detail.

Small defects (#262, 0.4.97): unreadable fragment ignored by the guest page; `https_only(true)` on the TIDAL, other-service links and APK clients (the `tidal/mod.rs` test that targets `127.0.0.1` over HTTP has its own client); `strip_decorations` looks for "feat." with `to_ascii_lowercase`. Android's `FileProvider` is **kept**: wry (`RustWebChromeClient.createImageFile`) uses it for a photo taken from an `<input type="file">`; its paths are narrowed to `getExternalFilesDir(Pictures)` (`res/xml/file_paths.xml`).

## Audit (0.4.57, #144 to #148)

Security and performance audit of 2026-11-03, by reading the code. Nothing serious on the security side (strict CSP, `core:default` capability, mobile plugin with no command for the UI, manifest server token, Keystore). Fixed: signed URLs in network errors (#144), Android sign-in page and backup (#145), Android rendering (#146), position and saving in the background (#147), token refresh and resolution (#148). Ruled out, with the reason in the issue: filtering media session controllers (#145, risk for the Buds and the lock screen), HTTP compression (#148, TIDAL does not compress). Not handled: virtualization of long lists on Android (to be measured on the S21 first).
