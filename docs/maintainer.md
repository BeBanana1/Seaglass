# Maintainer setup (for Claude)

Instructions specific to the maintainer's machine and accounts, imported by [`CLAUDE.md`](../CLAUDE.md). A contributor's session can ignore this file: without the `gitea` MCP tools or the variables below, none of it applies.

## Working with the maintainer

- The maintainer writes in French: answer them in French. Everything written to the repository stays in English (see "Language" in `CLAUDE.md`).
- **Do not accept a proposal from the maintainer as is**: for every proposal (feature, tool, workflow, architecture), first say whether there is a better way or a more standard practice, with the reasons and a recommendation, then go ahead with what they confirm.
- Claude may commit and push on its own, saying so in its message. Commits stay under the maintainer's git identity, with the `Co-Authored-By` line; merging PRs is up to the maintainer.

## Gitea access

- Repository: `bebanana/Seaglass` on `https://git.bebanana.ch` (Gitea 1.27, `origin`). The instance requires authentication everywhere, including the API.
- Access through the official `gitea-mcp` MCP server (v1.11.0, installed in `%LOCALAPPDATA%\Programs\gitea-mcp\`), declared in `.mcp.json`. It acts as the dedicated `claude` account (write collaborator on the repository, not admin: it cannot change the repository's settings), so that its issues, comments and PRs stand apart from the maintainer's and notify them. That account's token comes from the user environment variable `GITEA_CLAUDE_TOKEN` (scopes `write:issue`, `write:repository`, `read:user`): never write it to a file or display it.
- The maintainer's personal token is not used by Claude: if the Gitea tools fail (missing variable, revoked token), report it rather than falling back to `GITEA_ACCESS_TOKEN` or calling the API with another token. Check: `get_me` must return `claude`.
- Useful tools: `list_issues`, `issue_read` (`method: get` / comments), `issue_write` (comment, labels), `pull_request_write`.

## Machines and devices

- Development and releases on the maintainer's Windows PC (signing keys, release scripts: [`release.md`](release.md)).
- Android tests on a Samsung Galaxy S21 over USB debugging ([`android.md`](android.md), "Testing on the S21").
- Linux builds and tests in WSL ([`linux.md`](linux.md)).
- Update hosting and the Jam relay on the maintainer's server ([`release.md`](release.md), [`../relay/README.md`](../relay/README.md)); the CI runner on the same server ([`../ci/README.md`](../ci/README.md)).
