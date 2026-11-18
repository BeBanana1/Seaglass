# Seaglass CI runner

Gitea Actions runner dedicated to the repository, on the server, with its own job image
(Node, pnpm, Rust and the Tauri libraries preinstalled) and a cache that
works. The workflow (`.gitea/workflows/ci.yml`) targets the `seaglass` label.

## Installation

1. Copy this folder to the server, for example to `~/seaglass-runner/`:

   ```powershell
   scp -r ci <server>:~/seaglass-runner
   ```

2. On the server, create the `.env` from `.env.example`. The token comes from the
   repository: **Settings → Actions → Runners → Create new runner**. Take it at
   repository level (not instance level): the runner mounts the Docker socket, it must
   only serve Seaglass.

3. Build the job image, then start the runner:

   ```sh
   docker compose --profile build build
   docker compose up -d
   ```

4. Check that the `seaglass` runner shows up online in Gitea, with the
   `seaglass` label.

## Updating the image

New Debian base, new pnpm major, or Rust too old (the workflow
updates it on every run, but that slows it down): change the tag everywhere, then
rebuild. Same for `cargo-audit` (#263): an image built before it was
added works, but the "Dependencies" step skips the Rust audit with a
warning. The workflow no longer compiles it itself: installed in the container,
outside the Rust cache, it was recompiled on every run (+1 min 44 s). For it alone, keeping
the tag and rebuilding is enough (step 2).

1. `SEAGLASS_CI_TAG` in `.env` and the label in `config.yaml` (same value).
2. `docker compose --profile build build`, then `docker compose up -d` (the runner
   rereads its config on restart).
3. Remove the old image: `docker image rm seaglass-ci:<old tag>`.

## Troubleshooting

- **Jobs waiting forever**: the runner is not online, or its label does not
  match `runs-on: seaglass`.
- **`ETIMEDOUT` on the cache**: the jobs are not on the `seaglass-ci` network
  (`container.network` in `config.yaml`). The cache is optional: the run still passes,
  but loses 5 min per cache action.
- **`pull access denied` for `seaglass-ci`**: the image was not built, or
  its tag differs from the label.
- **Job killed (code 137, `Killed`)** during the Rust build: it went over the memory
  limit of `container.options` in `config.yaml` (4 GB). Raise it to `--memory=5g`
  rather than removing the limit: without it, a rustc can freeze the whole VM. After
  changing `config.yaml`, recreate the runner:
  `docker compose up -d --force-recreate runner`.
- **Unknown config key** after an act_runner update: compare with
  `docker run --rm --entrypoint act_runner docker.io/gitea/act_runner:<version> generate-config`.
