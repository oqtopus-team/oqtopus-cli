
# Development Environment Setup

This guide explains how to set up the development environment for contributing to Python Project Template.  
The project provides a **Makefile** to simplify common development tasks.

## Prerequisites

Install the following tools before starting development.

| Tool                                        | Version | Description                        |
| ------------------------------------------- | ------- | ---------------------------------- |
| [Python](https://www.python.org/downloads/) | >=3.13  | Python programming language        |
| [uv](https://docs.astral.sh/uv/)            | >=0.10  | Python package and project manager |

Clone the repository:

```shell
git clone https://github.com/oqtopus-team/oqtopus-cli.git
cd oqtopus-cli
```

## Project Structure

The repository is organized as follows:

```text
oqtopus-cli/
├─ docs/          # Documentation sources (MkDocs)
├─ .vscode/       # VSCode settings
├─ .github/       # GitHub workflows and repository settings
├─ pyproject.toml # Project configuration and dependencies
├─ Makefile       # Development commands
├─ mkdocs.yml     # MkDocs configuration
├─ uv.lock        # Locked dependency versions
└─ README.md      # Project overview
```

## Installing Dependencies

Install the project dependencies and set up the local development environment:

```shell
make install
```

This command performs the following:

- Installs all dependencies via `uv`.
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

This project uses [MkDocs](https://www.mkdocs.org/) to generate the HTML documentation and
Start the documentation server with:

```shell
make docs-serve
```

Open the documentation in your browser at [http://localhost:8000](http://localhost:8000).
