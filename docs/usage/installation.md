# Installation

Install the `oqtopus` command with the installer script:

```bash
curl -LsSf https://raw.githubusercontent.com/oqtopus-team/oqtopus-cli/main/scripts/install.sh | sh
```

By default, the installer places `oqtopus` in:

```text
~/.local/bin
```

Make sure this directory is on your `PATH`.

## Install A Specific Version

```bash
curl -LsSf https://raw.githubusercontent.com/oqtopus-team/oqtopus-cli/main/scripts/install.sh | sh -s -- --version v1.0.0
```

If `--version` is omitted, the installer resolves the latest stable `vX.Y.Z`
release.

## Install To A Custom Directory

```bash
curl -LsSf https://raw.githubusercontent.com/oqtopus-team/oqtopus-cli/main/scripts/install.sh | sh -s -- --bin-dir ~/.local/bin
```

## Shell Completion

The installer attempts to place shell completion files in standard user-local
locations for bash, zsh, and fish. Completion setup failures are warnings and do
not make the installation fail.

The installer does not modify shell startup files such as `.bashrc`, `.zshrc`,
`.profile`, or `config.fish`.

See [Shell Completion](./shell-completion.md) for manual setup.

## Supported Platforms

OQTOPUS CLI currently supports Linux and macOS.

Windows is not supported.

## Prerequisites

### Docker

Docker is required in two cases:

**Cloud-local template:** `oqtopus cloud-local start db` starts the database
and object storage via Docker Compose:

```bash
oqtopus cloud-local start db
```

**Backend template:** `oqtopus backend install engine` builds the `sse_runtime`
Docker image as part of the installation:

```bash
oqtopus backend install engine
oqtopus backend install all
```

To defer the `sse_runtime` build, use `--skip-sse-build` and build it later:

```bash
oqtopus backend install engine --skip-sse-build
oqtopus backend build sse-runtime
```

### Git

`git` is required when installing any component from a branch:

```bash
oqtopus cloud-local install cloud branch:develop
oqtopus backend install engine branch:develop
```

`git` is not required for release installs.
