# Seaglass

Unofficial TIDAL desktop client, with a fully custom UI and lossless / bit-perfect playback handed to mpv.
Personal project, Windows first.

> Status: current version in `VERSION`, log in [`CHANGELOG.md`](CHANGELOG.md) (a published version has a `v<version>` tag). Phases 0 to 5 shipped (WASAPI exclusive mode in 0.5.0, #245). Android app in progress (#24, [`docs/android.md`](docs/android.md)). Linux version in progress (#17, [`docs/linux.md`](docs/linux.md)). Last.fm postponed, not before the user asks for it.

This file only keeps what matters for every task. Implementation notes live in `docs/`: see "Documentation" below and read the area's file before touching it.

## Non-negotiable principles

1. **Streaming only.** No downloading, exporting or caching of audio streams on disk. No DRM circumvention: if a stream is protected by Widevine, it is not played, and a clear error is raised.
2. **Rust is the source of truth.** The UI has no business logic: it displays the state received from the backend and sends commands. It never talks to TIDAL or mpv directly.
3. **Everything TIDAL-related lives in `src-tauri/src/tidal/`.** It is an undocumented private API that can change at any time: the rest of the code only depends on the types in `tidal::models`, never on raw JSON formats.
4. **No secrets in the repository.** Client ID / secret in `.env` (gitignored) or environment variables. User tokens in the Windows Credential Manager through the `keyring` crate. Never log a token, even truncated.
5. **No TIDAL name or logo** in the app name, icon or window title. The notice "Unofficial client, not affiliated with TIDAL" appears in Settings › Application and in the README.

## Stack

| Layer | Choice |
|---|---|
| App | Tauri 2 |
| Backend | Rust stable, `tokio`, `serde`, `reqwest` (rustls), `thiserror` + `anyhow`, `tracing` |
| Frontend | Svelte 5 + TypeScript + Vite, `pnpm`, `vitest` tests, Prettier (+ `prettier-plugin-svelte`) |
| Audio | mpv driven over JSON IPC (named pipe), eventually a Tauri sidecar |
| Storage | SQLite (`rusqlite`) for the queue, settings and metadata cache |
| Secrets | `keyring` (Credential Manager on Windows) |
| Integrations | Windows SMTC (`windows` crate), MPRIS on Linux (`zbus` 5, already pulled in by the plugins, #312), notification area icon (`tauri`, `tray-icon` feature), Discord presence (hand-written IPC, #215), Last.fm (later) |
| Updates | `tauri-plugin-updater` (signed NSIS installer, `latest.json` manifest) |
| System | `tauri-plugin-autostart` (launch at startup, driven from Rust only), `tauri-plugin-single-instance` (single instance, #83), `tauri-plugin-opener` (links in the browser, from Rust only, #188), `tauri-plugin-global-shortcut` (global shortcuts, from Rust only, #413), `qrcode` without its default features (Jam QR code as SVG, #25), `tokio-tungstenite` (connection to the Jam relay, #25; with `rustls`, `rustls-platform-verifier` and `futures-util` as direct dependencies, already compiled by `reqwest`) |

Ask before adding a dependency that is not in this list.

## Target layout

```
seaglass/
├─ CLAUDE.md
├─ CHANGELOG.md              # version log
├─ docs/                     # implementation notes per area (see "Documentation")
├─ .github/                  # CI (workflows/ci.yml, GitHub Actions), issue and PR templates
├─ spike/                    # phase 0: throwaway Python scripts
├─ relay/                    # Jam relay (#25): separate crate, Docker behind Caddy
├─ site/                     # project website (#517): static pages, scripts/publish-site.ps1
├─ src/                      # Svelte frontend
│  ├─ lib/ipc.ts             # typed wrappers for Tauri commands and events
│  ├─ lib/transport.ts       # real invoke/listen, or a mock backend outside Tauri (dev)
│  ├─ lib/mock/              # mock backend + fixtures.json (gitignored) to preview the UI
│  ├─ lib/stores/            # mirror of the backend state (player, queue, library, ui, nav)
│  ├─ lib/i18n.svelte.ts     # current language and `t` dictionary; strings per area in lib/i18n/
│  ├─ components/            # including backdrops/ (per-theme backgrounds)
│  ├─ themes/                # acidwave.css, surreal.css (overrides per component class)
│  └─ views/                 # Home, Search, Album, Artist, Playlist, Library, Folder
└─ src-tauri/
   ├─ mpv/seaglass.conf        # mpv config dedicated to the app
   └─ src/
      ├─ lib.rs              # Tauri setup, task startup
      ├─ content.rs          # filter on offered content (explicit, #368)
      ├─ engine.rs           # playback engine contract (`Op`, `Event`, `Engine` trait)
      ├─ media3.rs           # Android Media3 engine (plugin `plugins/seaglass-mobile`)
      ├─ apk_update.rs       # Android app updates (`android.json` manifest, APK)
      ├─ jam/                # Jam (#25): api.rs (shared rules), server.rs (local network), relay.rs (online), page/, remote/ (remote control, #336)
      ├─ logs/               # on-disk log, crash reports (#192)
      ├─ i18n.rs             # effective language, translated messages (`tr`, `trf!`), TIDAL locale
      ├─ commands/           # auth.rs, player.rs, catalog.rs, settings.rs: validate input then delegate
      ├─ tidal/              # auth.rs, api.rs, catalog.rs, home.rs (paged home), playlist.rs (cached pages), mix.rs (mixes, radios), hires.rs + twin.rs (hi-res editions), credits.rs, raw.rs (raw JSON), paging.rs (paged lists), models.rs, cache.rs, manifest.rs (DRM), soundtracks.rs (curated soundtracks, #399), folders.rs (playlist folders, #331)
      ├─ player/             # actor.rs, queue.rs, state.rs
      ├─ mpv/                # engine.rs (mpv engine), process.rs, ipc.rs, events.rs, serve.rs (MPDs served over local HTTP)
      ├─ store/              # SQLite (including snapshots.rs: copies of lists)
      ├─ handoff.rs          # playback handoff with the linked computer, phone side (#414)
      ├─ foreign/            # links from other services: Jam (#229, links.rs, meta.rs, matching.rs), playlist import (#328, playlists.rs, import.rs)
      └─ integrations/       # mod.rs (NowPlaying, Observers), smtc.rs, mpris/ (Linux), tray.rs, discord/ (Discord presence), listenbrainz/ (scrobbling, #431)
```

## Player architecture

- **Single actor.** One tokio task owns `Player` (queue + state). Everything goes through an `mpsc::Sender<Cmd>`: UI commands, engine events, SMTC. No `Mutex` on the player state.
- **Playback engine** (`engine.rs`, 0.4.42, #24): the actor only sends semantic operations (`Op::Play`, `Preload`, `RemoveNext`, `DropPrevious`, `Seek`, `LoopCurrent`, `Volume`, `CurrentGain`…) and receives already translated `Event`s (`Position`, `Advanced`, `Ended(End::Eof | OutputFailed | Error)`, `Stopped`…). `mpv/engine.rs` (`MpvEngine`) translates both ways (`commands`, `translate`, pure tested logic; `cargo test live_engine_window -- --ignored --nocapture` with a real mpv). No mpv JSON command outside `mpv/`. Android's Media3 engine implements the same `Engine` trait. `AudioDevice` and `CacheSize` stay in `mpv` (desktop settings).
- **Queue** (`player/queue.rs`):
  - `context: Vec<TrackId>` (original album / playlist), `order: Vec<usize>` (play order, shuffled or not), `pos`
  - `manual: VecDeque<TrackId>` takes priority over the context ("Play next" = `push_front`, "Add to queue" = `push_back`)
  - `history: Vec<TrackId>` for "Previous", independent of shuffle
  - `repeat: Off | All | One`
  - Shuffle = Fisher-Yates over the remaining tracks, current track first. Turning shuffle off restores the original order at the current track.
- **mpv window of at most 2 entries**: current track + next one. Stream URLs are signed and expire, so nothing further is ever loaded.
  - Preload the next one (`loadfile <url> append`) when `duration - time_pos < 30 s`.
  - On `playlist-pos == 1`: advance the queue, `playlist-remove 0`, `next_loaded = false`.
  - **Any change to the queue** (play next, shuffle, removal, repeat) → `playlist-remove 1` if a next track is loaded, then `next_loaded = false`.
- **Repeat One** = `loop-file=inf` on the mpv side, no reload.
- **Previous**: if `time_pos > 3 s`, seek to 0, otherwise the last track of `history` with `loadfile … replace`.
- **Playback error** (`end-file` reason `error`): request a URL again once, then skip to the next track and emit a `toast` event.
- **To the UI**: `player-state` event emitted only when the state changed, with `time_pos` limited to about 4 Hz. The UI interpolates the progress bar with `requestAnimationFrame`. The queue list goes out in a separate `queue-state` event, emitted only when the queue changes (otherwise it would be resent 4 times per second).
- **Persistence**: queue and position saved in SQLite (2 s debounce), restored paused at startup.

## Roadmap

0. ~~**Spike**~~ ✅: Python script `spike/play.py` with `tidalapi`: sign-in, fetching a track's manifest, playback in mpv. DRM-free FLAC streams confirmed with the PKCE client.
1. ~~**Skeleton**~~ ✅: Tauri + Svelte, `mpv/` module (process + IPC + events), `login` (PKCE), `search`, `play`, `toggle_pause`, `seek` commands. Minimal player actor (one track at a time).
2. ~~**Queue**~~ ✅: `queue.rs` (tested), gapless, retry on error, repeat One through `loop-file`, SQLite persistence.
3. ~~**Views**~~ ✅: home, multi-type search, album, artist, playlist, library (playlists, albums, artists, tracks).
4. ~~**Integrations + UI**~~ ✅: SMTC; Seaglass / Studio / Atelier / Acidwave / Surrealism themes, "Now playing" view, signal path, keyboard shortcuts. Discord presence in 0.4.92 (#215); Last.fm postponed (not before the user asks for it).
5. ~~**Polish**~~ ✅: NSIS installer, bundled mpv + Job Object, signed updates, release script; update hosting (v0.4.4 published); CI; mixes and radios (0.4.8); WASAPI exclusive mode (0.5.0, #245).

## Documentation

Implementation notes, read on demand: verified endpoints, measurements, pitfalls, classes overridden by themes, procedures.

| File | Content | Read before touching |
|---|---|---|
| [`docs/player.md`](docs/player.md) | mpv (options, cache, normalization, output device, AVX2), manifests and pipe, queue and persistence, queue commands, volume, sleep timer, automatic radio, listening history, hi-res edition | `player/`, `mpv/`, `engine.rs`, `store/` |
| [`docs/tidal.md`](docs/tidal.md) | spike results, sign-in and DRM guard, catalog endpoints, feed, mixes, lyrics, library writes, "Save for later", track info and credits, backend performance | `tidal/`, `commands/catalog.rs`, `commands/collection.rs` |
| [`docs/interface.md`](docs/interface.md) | navigation, settings, shortcuts, preview without Tauri (URL parameters), CSP, pages and lists, context menu, pasted links, list performance, language | `src/` outside themes, `commands/settings/`, `i18n.rs` |
| [`docs/themes.md`](docs/themes.md) | tokens, backgrounds, accent color, cascade, notes on each theme, overridden classes | `src/themes/`, `app.css`, `components/backdrops/`, and before renaming a component class |
| [`docs/system.md`](docs/system.md) | SMTC, MPRIS, Discord presence, global shortcuts, notification area, launch at startup, logs and crash reports | `integrations/` (desktop), `logs/` |
| [`docs/linux.md`](docs/linux.md) | what differs on Linux (audio output, stopping mpv, exclusive mode), building and testing in WSL | desktop `cfg(not(windows))` code, a Linux package |
| [`docs/android.md`](docs/android.md) | Android app status, Media3 engine, media session, native bridge, secrets, touch layout, APK distribution, building and testing on the S21 | `media3.rs`, `android.rs`, `plugins/seaglass-mobile/`, `gen/android/`, `data-platform="android"` styles |
| [`docs/jam.md`](docs/jam.md) | local and online Jam, guest API, safeguards, relay, links from other services, remote control | `jam/`, `foreign/`, `relay/` |
| [`docs/release.md`](docs/release.md) | NSIS installer, bundled mpv, updates, signing, release scripts, release on the forge, CI | `scripts/`, `updater.rs`, `tauri*.conf.json`, `.github/` |
| [`docs/audits.md`](docs/audits.md) | past audits and what they ruled out | a security or performance question already settled |

## Commands

PowerShell on Windows.

```powershell
# Spike
python -m venv spike\.venv; spike\.venv\Scripts\Activate.ps1
pip install -r spike\requirements.txt
python spike\play.py login --pkce
python spike\play.py search "<query>"
python spike\play.py probe <id>
python spike\play.py play <id> --exclusive

# App (from phase 1)
pnpm install
pnpm tauri dev
cd src-tauri; cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test
cargo test live_catalog -- --ignored --nocapture   # real endpoints (session required)
cargo test dump_ui_fixtures -- --ignored          # UI preview data
pnpm format                                       # Prettier on src/ (CI: pnpm format:check)
pnpm check
pnpm test                                         # frontend tests (vitest)
pnpm release                                      # signed installer in release/ (see docs/release.md)
pnpm release:all -Publish                         # Windows + Android + Linux (WSL), tag, Gitea release
```

## Conventions

- Frontend: Prettier (`pnpm format`, width 100, `.prettierrc.json`) must pass `pnpm format:check`, run by CI. `.editorconfig` covers the other files. The commit that formatted everything is listed in `.git-blame-ignore-revs` (`git config blame.ignoreRevsFile .git-blame-ignore-revs`).
- Rust: `cargo fmt` and `clippy -D warnings` must pass. No `unwrap()` / `expect()` outside tests and initialization. Typed errors with `thiserror` in modules, `anyhow` only at the boundaries.
- Tauri commands return `Result<T, String>` or a serializable error, never panic.
- Types shared between UI and Rust are defined on the Rust side and duplicated in `src/lib/ipc.ts` (consider `tauri-specta` if it becomes painful).
- Unit tests are mandatory for `player/queue.rs` (shuffle, repeat, manual queue, previous): it is pure logic, with no mpv or network.
- **Language**: identifiers, code, docs (`*.md`), comments, `tracing` log messages and untranslated technical error details, commit messages, issues and PRs in English. Many code comments and log messages are still in French: translate those of a file when you touch it, never in a mass change. UI strings stay translated in `src/lib/i18n/` and `src-tauri/src/i18n.rs` (French and English). `CHANGELOG.md` is in English too: the app shows it to everyone, whatever its language.
- Commits follow Conventional Commits (`feat(player): …`).- **Version**: `VERSION` at the root is the source of truth, shown in the app header. `0.<phase>.<patch>` before 1.0: patch for every change set **that modifies the shipped app** (code, config, dependencies), minor at the end of a phase. No version change for documentation alone (`*.md`), otherwise every doc fix would become an "update" once the updater is in place. CI checks it ("Version check" in `docs/release.md`). `src-tauri/Cargo.toml` must be updated at the same time (`build.rs` fails the build otherwise); `tauri.conf.json` has no version and takes Cargo's.
- License GPL-3.0 (`LICENSE`).

## Issues and pull requests

- **Repository**: [`BeBanana1/Seaglass`](https://github.com/BeBanana1/Seaglass) on GitHub (issues, PRs, CI, releases). The former Gitea repository is a private archive (`docs/maintainer.md`): a `#n` up to 528 in a commit older than 2026-11-22 refers to a Gitea issue or PR, not a GitHub one.
- **Labels**: `platform/android` | `platform/windows` | `platform/linux` (one at most; none = all), `type/bug` | `feature` | `improvement` | `perf` | `security` | `chore` | `docs` (one), `area/player` | `tidal` | `ui` | `theme` | `library` | `release` | `ci` (several allowed), `status/needs-info` | `blocked` | `wontdo` | `duplicate` (one at most; GitHub does not enforce it). Every issue created or handled gets at least one `type/` and one `area/`; `status/needs-info` when it waits for an answer from the maintainer or the reporter, removed once they answered; `status/wontdo` on a rejected proposal.
- To handle an issue: read it with its comments, post a comment with the plan before coding, work on an `issue-<n>-<topic>` branch, reference the issue in the commit (`fix(ui): … (#8)`) and open a PR to `main` with `Closes #<n>`. Answer in the issue when a question comes up rather than guessing.
- **Several issues in a row**: open each issue's PR as soon as its branch is verified and pushed, before moving to the next one, so that CI jobs spread out instead of piling up at the end. Each change set bumps `VERSION` and `CHANGELOG.md`: branch off the previous branch rather than `main` (stacked PRs, otherwise they conflict) and state the merge order in each description.

## Way of working (for Claude)

- **Subagents** (`.claude/agents/`, on Sonnet): `explorer` for a read-only search where only the conclusion matters, `verifier` for fmt, clippy, tests, `pnpm check` / `pnpm test` and the version check with a report limited to errors. Design and edits stay in the main session.
- Provide **complete files** when modified, not partial patches.
- Shell commands in PowerShell.
- Before implementing anything that depends on a TIDAL endpoint, check its real behavior (spike or test request) rather than assuming it.
- Keep modules small: if a file goes past about 400 lines, propose a split.
- **Documentation**: this file stays short. What is learned while handling an issue (verified endpoint, measurement, pitfall, decision and its reason) goes into the area's file in `docs/`, in the same PR. Fix or remove what is outdated rather than appending.
- **Changelog**: every `VERSION` change adds its entry at the top of `CHANGELOG.md` (one line, with the issue number; platform prefixes `Android:`, `Windows:`, `Linux:`, `Desktop:`). After a release, the title of the tagged version gets "(published <YYYY-MM-DD>)": the app's parser reads these exact markers.
- Update the "Status" line and the roadmap of this file at the end of each phase.
- `CONTRIBUTING.md` restates the conventions for human contributors, `SECURITY.md` the reporting policy, `.github/ISSUE_TEMPLATE/` and `.github/PULL_REQUEST_TEMPLATE.md` the forms: update them when a convention here changes.

## Maintainer setup

Accounts, tools and devices specific to the maintainer (GitHub access as BeBanana1 with a signature, Gitea archive, answering in French, test devices) are in `docs/maintainer.md`, imported here:

@docs/maintainer.md
