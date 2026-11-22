# Maintainer setup (for Claude)

Instructions specific to the maintainer's machine and accounts, imported by [`CLAUDE.md`](../CLAUDE.md). A contributor's session can ignore this file: without the variables below, none of it applies.

## Working with the maintainer

- The maintainer writes in French: answer them in French. Everything written to the repository stays in English (see "Language" in `CLAUDE.md`).
- **Do not accept a proposal from the maintainer as is**: for every proposal (feature, tool, workflow, architecture), first say whether there is a better way or a more standard practice, with the reasons and a recommendation, then go ahead with what they confirm.
- Claude may commit and push on its own, saying so in its message. Commits stay under the maintainer's git identity (`BeBanana`, BeBanana1's noreply address, set in the repository's local git config), with the `Co-Authored-By` line; merging PRs is up to the maintainer.

## GitHub access

- Repository: [`BeBanana1/Seaglass`](https://github.com/BeBanana1/Seaglass) (public, `origin`), since 2026-11-22. Issues, PRs, CI and releases all live there.
- Claude acts as the maintainer's **BeBanana1** account (their choice: no separate bot account), through `gh` with the fine-grained token in the user environment variable `GITHUB_CLAUDE_TOKEN`, passed per command: `$env:GH_TOKEN = [Environment]::GetEnvironmentVariable('GITHUB_CLAUDE_TOKEN','User'); gh …`. Never write the token to a file or display it.
- **Never use the default `gh` login** of the PC: it is another personal account (Guenin04), unrelated to the project, whose name must not appear on it. Check before acting: `gh api user --jq .login` must return `BeBanana1`. If the variable is missing or the token is refused, report it rather than falling back to another login.
- **Signature**: every comment, issue or PR description Claude writes on GitHub ends with `— Bebanana's Claude companion`, so that the maintainer's own words and Claude's stay apart. It is the only signature: no "Generated with Claude Code" footer in PR descriptions (maintainer's choice, it would sign twice). Commits are not signed this way (they keep `Co-Authored-By`).
- Pushes go over SSH with the PC's key, registered on BeBanana1 (`ssh -T git@github.com` answers `Hi BeBanana1!`).

## Gitea archive

- The former repository on `https://git.bebanana.ch` (Gitea, private, sign-in required everywhere) is only an archive: the old issues and PRs up to #528, with the history **before** the 2026-11-22 rewrite (author addresses replaced before publishing on GitHub, so every commit hash changed; a `#n` up to 528 in an old commit refers to a Gitea issue or PR). A Gitea pull mirror keeps a private copy of the GitHub repository. Local remote: `gitea`. Nothing is pushed there anymore.

## Machines and devices

- Development and releases on the maintainer's Windows PC (signing keys, release scripts: [`release.md`](release.md)).
- Android tests on a Samsung Galaxy S21 over USB debugging ([`android.md`](android.md), "Testing on the S21").
- Linux builds and tests in WSL ([`linux.md`](linux.md)).
- Update hosting, the website and the Jam relay on the maintainer's server ([`release.md`](release.md), [`../relay/README.md`](../relay/README.md)).
