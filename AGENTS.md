# AGENTS.md

Instructions for coding agents working in this repository. Follow them unless the
user explicitly overrides them.

## What this repository is

**oqtopus-cli** is a Rust command line interface for creating and operating local
[OQTOPUS](https://github.com/oqtopus-team) environments (`backend`, `cloud-local`,
`manager`). The deliverable is a single executable named `oqtopus`,
published as a platform binary by `.github/workflows/release.yml`.

The CLI was ported from a single Bash script. That script (`bin/oqtopus`) and its
runtime fallback are **gone**: every route is native Rust. Documentation, comments,
or tooling that still describes a Bash CLI is stale — fix it when you touch it,
do not imitate it.

Alongside the crate, the repository ships the environment templates the CLI
installs (`templates/`), the POSIX-sh installer (`scripts/install.sh`), and the
MkDocs site published to <https://oqtopus-cli.readthedocs.io/>.

## Layout

```text
src/                     Rust crate (binary `oqtopus`, edition 2024)
  main.rs                signal setup, argv, process exit — nothing else
  cli.rs                 route table, dispatch, stream selection, exit codes
  backend/ cloud_local/ manager/   domains: command policy, service inventory, ordering
  args/environment/metadata/service/operations/versions/remote/archive/progress/lifecycle
                         shared mechanisms (no domain orchestration)
  text.rs                rendering of final results and errors
tests/characterization/  snapshot + behavior suite against the built binary
tests/cli.rs             Rust-only behavior that has no Bash-era counterpart
design/                  authoritative design records (read before large changes)
templates/<template>/    files copied into a user environment by `oqtopus init`
docs/                    MkDocs sources; docs/usage/command-reference.md is user-facing truth
scripts/install.sh       end-user installer (POSIX sh, not Bash)
```

Read `design/runtime_architecture.md` before changing module boundaries, process
startup, or text handling, and `design/rust_migration.md` for the recorded
compatibility decisions and their rationale. Those documents win over this file
on design questions; this file wins on workflow.

## Commands

```bash
cargo test --locked          # or: make test — full suite, always run before finishing
cargo test --locked --test characterization
cargo fmt                    # rustfmt defaults, no rustfmt.toml
cargo clippy --all-targets   # currently warning-free; keep it that way
make docs-lint               # pymarkdownlnt over docs/ only
make docs-build              # MkDocs build
make install                 # uv sync (docs tooling only) + git commit template
```

`make install` sets up **documentation** tooling (Python ≥ 3.13, uv ≥ 0.10). It is
not needed to build or test the CLI; there is no Python or Bash runtime code.

CI (`.github/workflows/rust-test.yml`) runs `cargo test --locked` on linux-gnu,
linux-musl, and macOS. Keep all three green: musl and macOS exist to catch libc,
allocator, and portability divergence, so avoid platform-specific assumptions.

## Hard constraints

These are compatibility contracts, not preferences. Breaking one is a coordinated
change, never a drive-by cleanup.

- **Downstream consumer boundary.** The OQTOPUS Manager application runs this CLI
  as a subprocess and parses its text output line by line: the `name: state` rows
  of `status` (including `(PID N)` and container annotations), the `key=value` rows
  of `info`, and the order, current-version marker, and annotations of `versions`.
  Exit status, the stdout/stderr split, and the environment-root layout
  (`.metadata`, `config/.env`, `logs/<service>/service.log`, `pids/`) are part of
  the same contract. If a change touches it, say so explicitly in the PR.
- **Incremental output.** Streamed commands flush progress as it happens, including
  updates without a trailing newline, and flush before handing a stream to a child
  process. Never rely on implicit line buffering; never hold the standard streams
  open through a spawned daemon. Snapshots of finished output cannot catch this.
- **Result data stays separate from rendering.** Commands return concrete result
  structs; `src/text.rs` renders them into a `Write` sink. Do not build output by
  formatting strings inside command logic — a future `--json` renderer must be able
  to serialize the same data.
- **Help text is byte-stable.** It is pinned by snapshots. Reformat it only as a
  deliberate, reviewed change.
- **Text is UTF-8.** Metadata and other text files use `String`/`&str`, and invalid
  UTF-8 is rejected at the read boundary — no lossy views, no parallel byte-oriented
  editing path. Successful output preserves the source bytes, including unknown
  keys, ordering, and CRLF. Binary payloads (archives, network bodies) stay bytes.
- **Argument parsing stays hand-written.** Adopting `clap` is a planned, separate
  change after the migration branch merges. Do not introduce it incidentally.
- **Prefer std and crates over shelling out.** `ps`, `readlink`, `id`, `curl`, `jq`,
  and `tar` are not dependencies. `uv` and Docker are real product behavior and are
  invoked directly (no `eval`, no shell-escaped command strings).
- **Service startup rules.** Mutual exclusion is a nonblocking OS lock on the
  persistent `pids/.<service>.start.guard` file, which is never deleted. The child
  publishes its own PID before `exec`. PID 0 is rejected. See the sequence diagram
  in `design/runtime_architecture.md` before editing `src/service.rs`.

## Tests

Every change ships with tests sufficient for its argument handling, main behavior,
errors, and externally observable results.

- `tests/characterization/` runs the built binary in a temporary sandbox
  (`harness::TestContext`) with controlled paths and locale, normalizing machine
  paths to `<TEST_ROOT>`. Never read the developer's real environment.
- Snapshots are `insta` snapshots under `tests/characterization/snapshots/`. They
  record the behavior the Bash CLI had, and are **compatibility evidence**. Review
  every `.snap.new` by hand and state why the change is correct; never accept
  snapshot churn mechanically (`cargo insta accept` over a whole run is not review).
- Use a snapshot when reviewing a whole artifact helps (command output, generated
  files). Use focused assertions for ordering, parsing, and validation semantics —
  and pin Manager-visible details (row order, `(PID N)`, current-version marker)
  with focused assertions *in addition to* the snapshot, so they read as
  load-bearing.
- **No network in tests.** Remote access is served from fixed fixtures via the
  `OQTOPUS_TEST_*` hooks in `src/remote.rs`, `src/init.rs`, and
  `src/cloud_local/lifecycle.rs`. Those hooks are inert unless
  `OQTOPUS_TEST_MODE` is set. Live GitHub access is a manual check, not a CI requirement.
- New characterization files need a `mod` line in `tests/characterization/main.rs`;
  `autotests = false`, so a new *test target* also needs a `[[test]]` entry in
  `Cargo.toml`.

## Style

- Rust: rustfmt defaults (4-space). Everything else follows `.editorconfig`
  (2-space, LF, final newline, UTF-8; tabs in Makefiles).
- Module-level `//!` doc comments state the module's responsibility. Comments
  explain current invariants and *why* — not what the Bash version used to do.
  Historical rationale belongs in `design/`, and only where it justifies a
  compatibility constraint.
- Code, comments, commit messages, and PR text are written in English.
- Keep domain policy in the domain module and reusable mechanism in the shared
  module. A command belongs under its domain, not in a catch-all lifecycle module.

## Commits, branches, and PRs

- Branches: `feature/xxx`, `bugfix/xxx`, `hotfix/xxx`, cut from and merged into `main`.
- Commits: Conventional Commits, one line, English, ≤72 characters, no emoji, no
  trailing period. `<type>(<scope>): <summary>` with
  `type` ∈ `feat|fix|docs|style|refactor|test|ci|chore` and `scope` ∈
  `cli|docs|infra|repo` (omit when unclear). Append `(#123)` for a related issue.
  PR labels are derived from this prefix by `.github/workflows/labeler.yaml`.
- PR title uses the same format; the description follows
  `.github/pull_request_template.md` (Ticket / Summary / Changes).
- Do not commit, push, or open a PR unless the user asks.

## Documentation

- User-facing behavior changes update `docs/usage/command-reference.md` and any
  affected page under `docs/usage/`. Do not document commands or options that are
  not implemented.
- `make docs-lint` scans `docs/` only; `README.md` and this file are not linted.
  `MD013`, `MD041`, `MD046`, and `MD060` are disabled (see `pyproject.toml`).
- Fenced code blocks carry an explicit language; Mermaid renders via
  `pymdownx.superfences`.

## Known gaps

Do not treat these as intentional; they are tracked work.

- `scripts/install.sh` still downloads the Bash-era source archive and extracts
  `bin/oqtopus`, which no longer exists. It must be rewritten against the release
  binaries before the migration branch merges into `main`.
- `README.md` and `docs/developer_guidelines/setup.md` still describe a Bash CLI
  and a Python-only development setup.
