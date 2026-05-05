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

OQTOPUS CLI v1.0.0 supports Linux and macOS.

Windows is not supported yet.

## Prerequisites

Install Docker before installing the `engine` backend component. OQTOPUS CLI
builds the `sse_runtime` Docker image during:

```bash
oqtopus backend install engine
```

and:

```bash
oqtopus backend install all
```

The Docker image build can take several minutes, especially the first time it
runs or after Docker cache cleanup.
