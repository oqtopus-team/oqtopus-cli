# Migrating From v1

OQTOPUS CLI v2 replaces the v1 shell-script CLI with a single native
executable. The commands, options, and environments stay the same, so most
users only need to reinstall the CLI.

## What Stays The Same

- Every v1 command and option, including help text and shell completion.
- The text output of `status`, `info`, and `versions`, which OQTOPUS Manager
  parses, as well as exit statuses and the stdout/stderr split. See
  [Known Differences In v2 Prereleases](#known-differences-in-v2-prereleases)
  for the exceptions that remain before v2.0.0.
- The environment layout: `.metadata`, `config/.env`, `logs/`, and `pids/`.
- Component releases installed in the shared data directory.

Existing environments work without running `oqtopus init` again. `.metadata`
files written by older v1 releases are upgraded automatically, as in v1.

Services started by v1 can be checked and stopped by v2, because both
versions use the same PID files.

## Upgrading

1. Install v2 with the installer. Until v2.0.0 is released, request a v2
   release explicitly:

    ```bash
    curl -LsSf https://raw.githubusercontent.com/oqtopus-team/oqtopus-cli/main/scripts/install.sh | sh -s -- --version v2.0.0-beta.1
    ```

    Use the same `--bin-dir` as the v1 installation, if you changed it, so the
    new executable replaces the old one. OQTOPUS Manager runs `oqtopus` from
    `PATH` and needs no configuration change.

2. Confirm the installed version:

    ```bash
    oqtopus version
    ```

3. Start a new shell so that the regenerated completion files are loaded.

Do not run v1 and v2 against the same environment at the same time. Each
version prevents concurrent starts of a service only among its own
invocations.

## Behavior Changes

- `oqtopus version` reports the version built into the executable and never
  contacts the network. The `OQTOPUS_CLI_VERSION` environment variable is no
  longer used.
- The CLI no longer needs `curl`, `jq`, or `tar` at runtime. `uv` and Docker
  are still required where v1 required them.
- `.metadata` and other text files must be valid UTF-8, and so must
  command-line arguments. Files with CRLF line endings are accepted.
- Prebuilt binaries are published for Linux (`x86_64`, `aarch64`) and macOS
  on Apple silicon. Intel-based macOS is not supported. See
  [Supported Platforms](./installation.md#supported-platforms).

## Known Differences In v2 Prereleases

`v2.0.0-beta.1` does not include the following v1.1.0 changes. They will be
included in v2.0.0:

- Services are started with `uv run --no-sync`, so starting a service does not
  synchronize its virtual environment.
- The cloud-local `db` is managed through `oqtopus-cloud-infra`, and
  `cloud-local status` reports `db: Partially running (...)` with the names of
  the running database and storage services.

If you rely on these behaviors, keep v1.1.0 until v2.0.0 is released.

## Rolling Back

Install a v1 release with `--version` to return to the shell-script CLI:

```bash
curl -LsSf https://raw.githubusercontent.com/oqtopus-team/oqtopus-cli/main/scripts/install.sh | sh -s -- --version v1.1.0
```

Environments remain usable by v1 after using v2.
