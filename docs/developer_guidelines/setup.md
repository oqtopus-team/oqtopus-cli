# Development Environment Setup

This guide explains how to set up the development environment for contributing to OQTOPUS CLI.
The CLI is a Rust crate that builds the `oqtopus` executable. The project provides a
**Makefile** to simplify common development tasks.

## Prerequisites

Install the following tools before starting development.

| Tool                                            | Version       | Description                                   |
| ----------------------------------------------- | ------------- | --------------------------------------------- |
| [Rust](https://www.rust-lang.org/tools/install) | latest stable | Builds and tests the CLI (edition 2024)       |
| [GNU Make](https://www.gnu.org/software/make/)  | —             | Runs the development commands in the Makefile |
| [cargo-insta](https://insta.rs/)                | —             | Reviews and updates test snapshots            |
| [Python](https://www.python.org/downloads/)     | >=3.13        | Builds and lints the documentation only       |
| [uv](https://docs.astral.sh/uv/)                | >=0.10        | Installs the documentation tooling            |

### Clone the Repository

Clone the repository:

```shell
git clone https://github.com/oqtopus-team/oqtopus-cli.git
cd oqtopus-cli
```

## Project Structure

The repository is organized as follows:

```text
oqtopus-cli/
├─ src/                # Rust sources of the oqtopus executable
├─ tests/              # Characterization and behavior tests
├─ templates/          # Files copied into environments by `oqtopus init`
├─ scripts/install.sh  # End-user installer (POSIX sh)
├─ design/             # Design records for the CLI
├─ docs/               # Documentation sources (MkDocs)
├─ .github/            # GitHub workflows and repository settings
├─ Cargo.toml          # Crate manifest
├─ pyproject.toml      # Documentation tooling dependencies
├─ Makefile            # Development commands
├─ mkdocs.yml          # MkDocs configuration
└─ README.md           # Project overview
```

Read `design/runtime_architecture.md` and `design/rust_migration.md` before changing
module boundaries or compatibility-sensitive behavior.

## Building And Testing

Build the CLI:

```shell
cargo build
```

The executable is written to `target/debug/oqtopus`.

Run the full test suite:

```shell
make test
```

This runs `cargo test --locked --features test-hooks`. The `test-hooks` feature lets
the tests serve remote data from fixtures instead of the network; release builds never
enable it.

Run only the characterization snapshots:

```shell
make test-characterization
```

Format and lint the code:

```shell
cargo fmt
cargo clippy --all-targets --features test-hooks
```

## Installing Documentation Tooling

Install the documentation dependencies and set up the local development environment:

```shell
make install
```

This command performs the following:

- Installs the documentation tooling via `uv`.
- Configures the Git commit message template.

## Documentation

### Lint Documentation

Run documentation linting:

```shell
make docs-lint
```

### Build Documentation

Build the documentation:

```shell
make docs-build
```

### Start the Documentation Server

This project uses [MkDocs](https://www.mkdocs.org/) to generate the HTML documentation.
Start the documentation server with:

```shell
make docs-serve
```

Open the documentation in your browser at [http://localhost:8000](http://localhost:8000).
