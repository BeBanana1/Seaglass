# Security policy

## Supported versions

Only the latest published version gets fixes (see [`CHANGELOG.md`](CHANGELOG.md); the app updates itself). Older versions are not patched.

## Reporting a vulnerability

**Do not open a public issue** for a vulnerability, and do not include exploit details in a public comment.

Instead, open an issue titled "Security contact request", with no details, and the maintainer will get back to you through a private channel. Then send:

- what is affected (version, platform: Windows, Linux or Android) and what an attacker could do;
- how to reproduce it;
- whether it is already public or known to others.

You will get an answer within a week. Once a fix is published, the issue can be described publicly, with credit if you want it.

## Scope

In scope, for example:

- leaking the TIDAL tokens (kept in the system credential store), signed stream URLs, or anything that should stay on the device;
- the local servers: the Jam (`jam/`), the remote control and the playback handoff (`jam/remote/`), the manifest server for mpv (`mpv/serve.rs`);
- the Jam relay (`relay/`) and the end-to-end encryption of the handoff tunnel (`handoff/sealed.rs`);
- the update chain: signature checks of the desktop updater and of the Android APK (`updater.rs`, `apk_update.rs`);
- anything that lets a web page, a Jam guest or another local process run commands in the app.

Out of scope:

- the TIDAL API itself, and the fact that Seaglass uses identifiers of TIDAL's Android client (documented in the README);
- the unsigned Windows installer (SmartScreen warning), already known;
- vulnerabilities in mpv, WebView2, WebKitGTK or Android System WebView, unless Seaglass makes them reachable in a way it should not; report those upstream.

Past audits and what they ruled out are listed in [`docs/audits.md`](docs/audits.md).
