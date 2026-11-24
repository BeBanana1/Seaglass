# Contributing to Seaglass

Thanks for your interest. Seaglass is a personal project kept by one maintainer: bug reports, ideas and pull requests are welcome, and every change goes through a review before it is merged.

## Before you start

- Read the five **non-negotiable principles** at the top of [`CLAUDE.md`](CLAUDE.md). In short: streaming only (nothing downloaded or cached on disk, no DRM circumvention), Rust is the source of truth (the UI has no business logic), everything TIDAL-related lives in `src-tauri/src/tidal/`, no secret in the repository, no TIDAL name or logo in the app's identity. A change that breaks one of them will not be merged.
- For anything bigger than a small fix, **open an issue first** and describe what you want to do: it avoids work that would be refused for a reason you could not know.
- New dependencies: the allowed stack is listed in `CLAUDE.md` › Stack. Ask in the issue before adding anything else.
- Before touching an area, read its notes in [`docs/`](docs/) (the table in `CLAUDE.md` › Documentation says which file covers what). They record verified endpoints, measurements and pitfalls.

## Setting up

Follow "Requirements (development)" and "Getting started" in the [README](README.md), including "TIDAL client identifiers". You need a TIDAL subscription to test playback.

Without Tauri, the interface runs on mock data in a browser: `pnpm dev`, then `http://127.0.0.1:1420/?theme=studio&route=album`. The URL parameters are listed in [`docs/interface.md`](docs/interface.md) ("Preview without Tauri"). It is the fastest way to work on the UI and the themes.

The website (`site/`, Astro + Starlight) is a separate package: `pnpm --dir site install`, then `pnpm --dir site dev` while writing a page. Before a pull request that touches it, `pnpm --dir site build` (CI runs it: it fails if a page has inline code the server's CSP would block) and `pnpm --dir site preview` to see the build under that CSP ([`docs/release.md`](docs/release.md), "Website").

## Checks

Everything must pass before a pull request (CI runs the same):

```powershell
cd src-tauri; cargo fmt; cargo clippy --all-targets -- -D warnings; cargo test
pnpm format
pnpm check
pnpm test
```

`pnpm format` runs Prettier on `src/`; CI runs `pnpm format:check` and fails on unformatted code. To keep `git blame` useful across the commit that formatted the whole frontend, run once: `git config blame.ignoreRevsFile .git-blame-ignore-revs`.

Tests marked `#[ignore]` (`cargo test live_… -- --ignored`) call the real TIDAL API or a real mpv, and some **modify the account**: read the test before running it.

## Conventions

- **Language**: code, comments, logs, docs, commits, issues and pull requests in English. Some code comments are still in French: translate those of a file you touch, but do not mass-translate unrelated files. User-facing strings go through the dictionaries (`src/lib/i18n/` for the UI, `tr` / `trf!` in Rust) with both French and English.
- **Rust**: no `unwrap()` / `expect()` outside tests and initialization; typed errors with `thiserror` in modules, `anyhow` only at the boundaries. Tauri commands return `Result<T, String>` or a serializable error, never panic.
- **Shared types** are defined in Rust and duplicated in `src/lib/ipc.ts`.
- **Tests**: required for pure logic (the queue in `player/queue.rs` in particular). Frontend tests sit next to their module (`src/lib/<module>.test.ts`).
- **Themes** override components by class name: before renaming a CSS class, check [`docs/themes.md`](docs/themes.md) and `src/themes/`.
- **Small modules**: past about 400 lines, split the file.

## Commits and pull requests

- Commits follow [Conventional Commits](https://www.conventionalcommits.org/) with a scope: `fix(player): …`, `feat(ui): …`, `docs: …`. Reference the issue: `fix(ui): … (#123)`.
- One branch per issue, named `issue-<n>-<topic>`, and a pull request to `main` with `Closes #<n>` (from a fork if you are not a collaborator). CI (GitHub Actions) must pass.
- Issue numbers before 2026-11-22 (up to #528, in old commits and docs) refer to the project's former Gitea repository, now a private archive, not to GitHub issues.
- **Version**: any change to the shipped app (code, config, dependencies) bumps the patch number in `VERSION` **and** in `src-tauri/Cargo.toml`, and adds a one-line entry at the top of [`CHANGELOG.md`](CHANGELOG.md) (prefix `Android:`, `Windows:`, `Linux:` or `Desktop:` when it concerns one platform only; the app shows this log to users). Documentation alone (`*.md`) does not change the version. CI checks this (`scripts/check-version.sh`).
- What you learn while working (verified endpoint, measurement, pitfall, decision and its reason) goes into the area's file in `docs/`, in the same pull request.

## AI assistants

The maintainer works with Claude Code: [`CLAUDE.md`](CLAUDE.md) and `.claude/` hold its instructions (the parts specific to the maintainer's machine and accounts are in [`docs/maintainer.md`](docs/maintainer.md)), and the issues, comments and pull requests it writes on the maintainer's account end with "— Bebanana's Claude companion". Contributions made with an assistant are welcome under the same rules; you stay responsible for what you submit and must have checked it.

## License

By contributing, you agree that your contribution is licensed under the [GPL-3.0](LICENSE), like the rest of the project.
