# Copilot Instructions for oqtopus-cli

## Repository Overview

**oqtopus-cli** is a Bash-based command line interface for setting up and operating a local [OQTOPUS](https://github.com/oqtopus-team) backend environment. The primary deliverable is a single executable shell script (`bin/oqtopus`) that users install system-wide. This repository is **documentation-first**: the source code lives in `bin/oqtopus` and `scripts/install.sh`, and the rest of the repository is MkDocs documentation published to [oqtopus-cli.readthedocs.io](https://oqtopus-cli.readthedocs.io/).

## Repository Structure

```text
oqtopus-cli/
├── bin/oqtopus              # Main CLI script (single Bash file, ~1000 LOC)
├── scripts/install.sh       # Installer script (downloads bin/oqtopus to ~/.local/bin)
├── templates/backend/       # Config/template files copied on `oqtopus init`
│   └── config/              # Per-component YAML config and logging config files
├── docs/                    # MkDocs documentation sources (Markdown)
│   ├── index.md             # Home page
│   ├── usage/               # End-user guides
│   └── developer_guidelines/ # Contributor guides
├── pyproject.toml           # Python project (docs tooling only, no Python runtime code)
├── mkdocs.yml               # MkDocs site configuration
├── Makefile                 # Developer workflow commands
├── .python-version          # Pins Python version (3.13)
├── .uv-version              # Pins uv version (0.10.9)
├── .github/
│   ├── workflows/labeler.yaml           # Auto-labels PRs from commit prefix
│   ├── instructions/                    # Copilot instruction fragments
│   │   ├── commit-message.instructions.md
│   │   └── pull-request-description.instructions.md
│   └── pull_request_template.md
└── uv.lock                  # Locked Python dep versions (managed by uv)
```

## Tech Stack

| Layer | Technology |
|---|---|
| CLI runtime | Bash (`set -euo pipefail`), targeting Linux and macOS |
| Docs tooling | Python ≥ 3.13, managed by [uv](https://docs.astral.sh/uv/) ≥ 0.10 |
| Docs framework | MkDocs + Material theme |
| Doc linting | `pymarkdownlnt` |

There is **no compiled language, no Python runtime code, and no test suite** in this repository. All logic is Bash.

## Development Environment Setup

```bash
# Install docs dependencies and configure git commit template
make install
# Equivalent to: uv sync --all-groups && git config --local commit.template .gitmessage
```

Prerequisites: Python ≥ 3.13, uv ≥ 0.10.

## Key Make Targets

| Command | Description |
|---|---|
| `make install` | Install Python deps (docs only) and set git commit template |
| `make docs-lint` | Lint all Markdown under `docs/` with `pymarkdownlnt` |
| `make docs-build` | Build MkDocs HTML site to `site/` |
| `make docs-serve` | Serve docs locally at http://localhost:8000 |
| `make help` | Show all available targets |

There are **no test commands** — the repository has no automated test suite.

## Linting Documentation

Always lint documentation changes before committing:

```bash
make docs-lint
```

Disabled `pymarkdownlnt` rules (see `pyproject.toml` `[tool.pymarkdown]`):

- `MD013` — line length limit
- `MD041` — first-line heading requirement
- `MD046` — code block style consistency
- `MD060` — reference link style consistency

## Code Style & Conventions

### Bash (bin/oqtopus, scripts/install.sh)

- Always use `set -euo pipefail` at the top of scripts
- 2-space indentation (see `.editorconfig`)
- Use `log()`, `warn()`, and `die()` helpers for output (defined in `bin/oqtopus`)
- Use `need_command` to assert external tool availability before use
- No shebangs other than `#!/usr/bin/env bash`

### Markdown (docs/)

- 2-space indentation for nested lists
- No trailing whitespace (except in `.md` files where `trim_trailing_whitespace = false`)
- All files should end with a newline
- Use fenced code blocks with explicit language identifiers (e.g., ` ```bash `)
- Mermaid diagrams are supported via `pymdownx.superfences`

### Python (pyproject.toml, tooling only)

- 4-space indentation
- Python ≥ 3.13 only
- Only `docs` dependency group exists — no application Python code

## Commit Messages

Follow [Conventional Commits](https://www.conventionalcommits.org/):

```
<type>(<scope>): <summary>
```

- **type**: `feat | fix | docs | style | refactor | test | ci | chore`
- **scope**: `cli | docs | infra | repo` (omit if unclear)
- Max 72 characters
- No emojis, no trailing period
- Append `(#123)` if there is a related issue

Examples: `docs(usage): add troubleshooting entry for Docker build`, `fix: correct service start order in start_all`

Labels are automatically applied to PRs based on the commit prefix (see `.github/workflows/labeler.yaml`).

## Pull Request Guidelines

PR title format: `<type>(<scope>): <summary>` (same as commit convention).

PR description must follow `.github/pull_request_template.md`:

```markdown
## Ticket
<!-- Link to the ticket / issue -->

## Summary
<!-- What and why (1–2 sentences) -->

## Changes
<!-- Bullet list of key changes -->
```

## Branch Naming Conventions

- `feature/xxx` — new feature
- `bugfix/xxx` — bug fix
- `hotfix/xxx` — urgent/critical fix

All branches are created from and merged back into `main`.

## Important Files to Understand First

When working on any task, read these files first:

1. `bin/oqtopus` — the entire CLI implementation (~1000 LOC Bash)
2. `docs/usage/command-reference.md` — authoritative user-facing command reference
3. `templates/backend/config/` — config templates copied into user environments on `oqtopus init`
4. `mkdocs.yml` — navigation structure of documentation site

## Backend Components and Services

The CLI manages three installable **components** (downloaded from GitHub releases):

| Component | Source Repo |
|---|---|
| `engine` | `oqtopus-team/oqtopus-engine` |
| `tranqu` | `oqtopus-team/tranqu-server` |
| `gateway` | `oqtopus-team/device-gateway` |

The `engine` component provides five **services**: `core`, `sse_engine`, `mitigator`, `estimator`, `combiner`. Together with `tranqu` and `gateway`, there are seven total managed services.

Start order: `gateway → tranqu → mitigator → estimator → combiner → sse_engine → core`
Stop order: reverse of start order.

## Known Errors and Workarounds

- **No Python application code**: `pyproject.toml` exists only for docs tooling. Do not add Python runtime code to this repository.
- **No test suite**: The repository has no `pytest`, `bats`, or other test framework. Validate Bash changes manually and lint docs with `make docs-lint`.
- **uv version pin**: The project uses uv `0.10.9` (`.uv-version`). If uv commands fail, ensure the correct version is installed.
- **Python version**: Must be Python 3.13. If `uv sync` fails due to version mismatch, check the `.python-version` file.
- **Markdown linting scope**: `pymarkdownlnt` only scans `docs/` (per `Makefile`). Files outside `docs/` (e.g., `README.md`) are not linted automatically.
