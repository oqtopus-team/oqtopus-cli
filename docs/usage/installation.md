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

The installer requires `curl` and `tar`. Installing v2 or later also requires
`sha256sum` or `shasum` to verify the download.

## Choosing A Version

If `--version` is omitted, the installer resolves the latest stable `vX.Y.Z`
release. Prereleases such as `v2.0.0-beta.1` are installed only when requested
explicitly:

```bash
curl -LsSf https://raw.githubusercontent.com/oqtopus-team/oqtopus-cli/main/scripts/install.sh | sh -s -- --version v2.0.0-beta.1
```

How the installer obtains the CLI depends on the major version:

| Version     | What the installer installs                                                            |
| ----------- | -------------------------------------------------------------------------------------- |
| v1          | The shell-script CLI, extracted from the tagged source archive                         |
| v2 or later | The prebuilt binary for your platform, verified against the release's `SHA256SUMS` file |

Until v2.0.0 is released, the latest stable release is a v1 release, so the
default installation is still the shell-script CLI. Request a v2 prerelease
with `--version` to try the new CLI. See
[Migrating From v1](./migrating-from-v1.md) before upgrading an existing
installation.

Running the installer again replaces the installed `oqtopus` with the requested
version. To roll back, install an earlier version with `--version`.
If the destination named `oqtopus` is a directory, the installer exits with an
error.

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

OQTOPUS CLI v2 publishes prebuilt binaries for:

| Platform | Architectures                                    |
| -------- | ------------------------------------------------ |
| Linux    | `x86_64`, `aarch64` (statically linked, any libc) |
| macOS    | Apple silicon (`arm64`)                           |

Intel-based macOS is not supported by v2. v1 runs on Linux and macOS on any
architecture where its tool dependencies are available.

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

!!! warning "SSE jobs require Docker Engine installed directly on Linux"
    SSE jobs are **not supported with Docker Desktop** (Mac / Windows,
    including WSL2 integration). Docker Desktop runs Docker inside a VM, so
    `docker0` does not exist on the host and the SSE container cannot reach
    `sse_engine`.

    SSE jobs work with **Docker Engine installed directly on Linux**. On
    Windows, install Docker Engine inside a WSL2 distribution (for example
    Ubuntu) instead of enabling Docker Desktop's WSL integration.

    Other job types do not use the SSE container. See
    [SSE Network Settings](./backend-configuration.md#sse-network-settings)
    for details.

To defer the `sse_runtime` build, use `--skip-sse-build` and build it later:

```bash
oqtopus backend install engine --skip-sse-build
oqtopus backend build sse-runtime
```
