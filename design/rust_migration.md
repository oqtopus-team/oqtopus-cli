# Rust CLI Migration Design

## Purpose

The OQTOPUS CLI is moving from Bash to Rust without a flag-day rewrite. Rust
becomes the user-facing entrypoint first, while commands that have not yet been
ported continue to run through the existing Bash CLI.

This document records rules that are specific to that staged migration. Normal
implementation choices remain with the person or agent implementing each
slice. Progress tracking belongs in issues, and command-specific compatibility
decisions belong in the pull request that makes them.

## Migration architecture

The Rust executable owns a small, explicit routing decision:

1. A migrated command is dispatched to its native Rust implementation.
2. Every other invocation is passed to the legacy Bash CLI.

Initially every invocation goes to Bash. The native surface then grows one
command or coherent subcommand area at a time.

Fallback must be selected before strict Rust-side parsing. Arguments belonging
to Bash must not be rejected, normalized, reordered, or reconstructed by Rust.
During the migration, unknown commands and unusual legacy argument forms
therefore default to Bash.

Native slices keep hand-written argument parsing while the fallback exists.
`clap` is deliberately not adopted during the port, because its handling of
malformed arguments and its generated usage text would not match the Bash
output that the snapshots pin. Moving to `clap` is a refactoring step after
the compatible port is complete, not part of any migration slice.

On supported platforms, fallback replaces the Rust process with `exec`. This
preserves the raw arguments, environment, working directory, standard streams,
signals, and exit behavior as closely as possible. Fallback is an entrypoint
concern and must not be invoked from native command logic.

The route inventory must be visible and reviewable. Changing a route from Bash
to Rust is an explicit migration step, not an incidental result of adding an
implementation.

The inventory is represented by explicit matches in the Rust entrypoint's
routing function. A route is native only when a match selects its Rust
implementation; the catch-all route remains Bash.

Routes are recorded per subcommand, such as `backend status`, not per top-level
command. A top-level command may therefore be partially native, with the
remaining subcommands still delegated to Bash. Closely related subcommands may
be migrated together in one slice when that is clearer.

Linux and macOS remain the supported platforms. During the migration, Linux is
the validation target and fallback uses `exec`; macOS validation is deferred
until the migration is complete. That deferral carries platform risk, so native
slices prefer the Rust standard library and crates over external tools such as
`ps` and `readlink` wherever the observed behavior allows.

Tests forbid fallback through an environment variable. When it is set, the
executable exits with a distinct exit code instead of running Bash. Tests for
migrated routes always set it.

## Downstream consumer boundary

The OQTOPUS Manager (`oqtopus-team/oqtopus-manager`) drives this CLI as a
subprocess. It is the primary non-human consumer, and its expectations form a
compatibility boundary that the migration must hold even where a behavior looks
like an incidental Bash detail.

The Manager executes `oqtopus` found on `PATH`, without a shell, with the
working directory set to an environment root and with no controlling terminal.
The boundary therefore covers:

- The invoked argv surface: `init`, and for `backend` and `cloud-local` the
  `status`, `info`, `versions`, `device-status`, `install`, `update`,
  `uninstall`, `build`, `start`, `stop`, and `restart` subcommands.
- The human-readable stdout of the read-only commands, which the Manager parses
  line by line: the `name: state` rows of `status` including their `(PID N)`
  and container annotations, the `key=value` rows of `info`, and the order,
  current-version marker, and annotations of `versions`.
- Exit status, and the separation of stdout from stderr for captured commands.
- Incremental output: streamed commands must flush progress as it happens
  rather than at process exit, and must not keep the standard streams open
  through a spawned daemon. This is a real porting hazard that snapshots of
  finished output cannot detect. Flush progress at notification boundaries,
  including updates without a trailing newline; do not rely on implicit line
  buffering. Additional buffering must not delay those notifications. Flush
  pending output before handing the same stream to a child process so that
  parent and child output retains its intended order. Bash `log` writes
  progress to stdout; text mode preserves that stream.
- The environment-root layout the Manager reads directly, such as
  `logs/<service>/service.log` and `config/.env`.

Structured (`--json`) output is an additive future direction, not a substitute
for this boundary: text output stays stable until a coordinated change is
agreed with the Manager.

Slices touching this boundary must state, in the implementing change, whether
the Manager's parsing still holds.

## Result data and rendering

As commands are migrated, keep result data separate from text rendering.
Commands such as `status` should return a command-specific struct containing
the observed state, including fields such as service names, states, and PIDs.
A separate text-rendering function accepts that result and a `Write` sink,
preserving the existing text format. A future JSON renderer can serialize the
same result without reconstructing data from display strings.

A wrapper around `println!` or an API accepting only formatted strings does
not establish this boundary. Native commands use concrete result types
(`VersionInfo` and `BackendInfo`) and text-rendering functions in `src/text.rs`.
Apply the same separation as further commands, such as `status`, are migrated.
Defer a shared output trait, JSON schemas, and `--json` implementation until
there is a concrete need for them.

Outputs whose compatibility contract requires original bytes remain verbatim.
For example, `BackendInfo` retains the metadata bytes, preserving
unknown fields and formatting. Future structured output can add a parsed view
without regenerating the existing text output from that view.

Progress notifications are separate from a command's final result. Introduce
a small reporter with explicit flushing when migrating the first command that
needs incremental progress. Preserve text-mode stdout/stderr behavior. In a
future JSON mode, reserve stdout for the structured result and route progress
and child-process logs separately so that they cannot corrupt it. Decide any
streaming JSON protocol when an actual consumer requires one.

## Command migration workflow

For each command or coherent subcommand slice:

1. Inspect the Bash implementation, documentation, and relevant usage,
   including whether the Manager invokes the slice.
2. Add the characterization cases needed to understand that slice.
3. Run those cases against Bash and review the resulting snapshots before
   implementing the Rust version.
4. Decide for each observed behavior whether to preserve it, intentionally
   change it, omit it as a Bash-specific detail, or leave it undecided.
   Behavior inside the downstream consumer boundary is preserved unless the
   change is agreed with that consumer and recorded.
5. Implement the Rust slice and add sufficient tests for its argument handling,
   main behavior, errors, and externally observable results.
6. Run migrated-route tests with Bash fallback forbidden.
7. Switch the explicit route to Rust only after the behavior decisions and
   tests are complete.

An undecided behavior blocks the route switch. Intentional compatibility
changes must be documented with the implementing change.

The characterization suite grows through this workflow. Completing broad
characterization coverage before Rust implementation begins is not a goal.

## Snapshot policy

Snapshots are compatibility evidence, not an automatic declaration that every
observed Bash behavior is permanent.

Use snapshots when reviewing a complete observable artifact is useful, such as
human-readable command output or generated files. Prefer focused assertions or
unit tests when they express parsing, ordering, validation, or other individual
semantics more clearly.

Help text is preserved byte-for-byte while a command is migrated and pinned by
snapshots. Improvements to help formatting are deferred until the surface they
describe is native.

Output the Manager parses is pinned twice: by a snapshot of the whole
artifact, and by focused assertions for the individual details its parsers
depend on, such as row order, the `(PID N)` form, and the current-version
marker. A snapshot alone records those details without stating that they are
load-bearing.

Do not preserve calls to `curl`, `jq`, `tar`, or other Bash implementation
details merely because the legacy CLI makes them. External calls that remain
part of product behavior, such as `uv` or Docker invocations, may be tested when
their arguments or effects matter.

Snapshot expectations must be produced from the Bash implementation before the
corresponding Rust implementation is written. Snapshot changes are reviewed;
they are never accepted mechanically.

Characterization tests run the Rust executable by default and compare its
output with the saved snapshots. While establishing snapshots before a port,
`make record-characterization` explicitly substitutes the Bash implementation
as the test subject. Normal test runs must not consult Bash for the expected
output.

Environment-dependent characterization tests create `.metadata` and the
required directory structure in a fresh temporary directory. They do not use a
developer's local environment. Machine-specific temporary paths are normalized
to `<TEST_ROOT>` before snapshot comparison; other volatile values such as PIDs
are either avoided, normalized, or checked with focused assertions.

The initial Rust wrapper deliberately has zero characterization snapshots.

## Test requirement

Every migrated slice must include tests sufficient to validate its argument
handling, main behavior, error handling, and externally observable results.
Compatibility-sensitive output and artifacts should use snapshots; other
behavior should use whichever focused tests communicate the expectation most
clearly.

Tests for a migrated route must be able to forbid Bash fallback. This prevents
a hybrid-entrypoint test from passing without exercising the Rust
implementation.

This design does not prescribe a test count, module layout, mocking strategy,
or a fixed ratio of snapshot, integration, and unit tests.

## Decided compatibility changes

- `version` prints only the version compiled into the executable and never
  contacts the network. The `OQTOPUS_CLI_VERSION` environment variable is no
  longer consulted. This is tested by direct comparison, not by snapshot, and
  packaging must inject the correct version at build time.
- Automatic migration of old `.metadata` keys, such as `env_root` to
  `environment_root`, is preserved for backward compatibility, including when
  it is triggered by read-only commands.
- Metadata parsing requires the `key=value` form. A bare `template` line is
  therefore reported as a missing template instead of reproducing Bash's
  accidental treatment of the whole line as its value.
- CRLF metadata is accepted. Validation ignores the carriage return through
  Rust's line parsing, while successful `info` output retains the original byte
  sequence. The legacy parser rejected these files because it included the
  carriage return in the field value.
- Command-line arguments are required to be valid UTF-8. Bash forwards arbitrary
  bytes, but no supported invocation needs them, and carrying `OsString` through
  routing and every command signature costs more than the capability is worth.
  An invocation with non-UTF-8 arguments therefore aborts in `std::env::args`
  before reaching either implementation, which is the standard library's own
  handling of the case. This is pinned by a Rust-only test rather than a
  characterization case, because Bash accepts the same invocation.
- `backend device-status` reports a failed write as
  `Error: failed to write device status file: <reason>` rather than the shell
  redirection error Bash emits. The exit status is unchanged, and no consumer
  parses this message.
- `cloud-local status` keeps reporting when a container-name lookup fails.
  Bash runs `docker ps` in a command substitution under `set -e`, so a failing
  lookup aborts the command before any line is printed. Rust omits the
  unreadable container from the `db: Running (...)` annotation and still prints
  every service row, which is what the Manager's line-by-line parsing expects.
- Native `versions` commands do not reproduce diagnostics emitted directly by
  a failing `curl` process. They preserve the CLI-owned error message and exit
  status; no downstream consumer parses the removed tool-specific diagnostic.
- Native `init` preserves the target environment directory when download or
  extraction fails, but cleans its private temporary extraction directory.
  Bash leaked that temporary directory by exiting before its cleanup command.
  Rust also reports CLI-owned extraction and copy errors without reproducing
  diagnostics emitted directly by `tar`, `find`, or `cp`.
- Native install and update commands use the shared Rust HTTP and archive
  implementations established by `versions` and `init`. They do not reproduce
  diagnostics or PATH requirements from `curl`, `jq`, or `tar`; CLI-owned
  errors and exit status remain unchanged. The `uv` and Docker processes remain
  external product behavior, including their argument order and inherited
  standard streams.
- Native `backend build sse-runtime` reads the real user and group IDs through
  the platform API instead of invoking `id`. This removes one shell-tool PATH
  dependency while preserving the Docker build arguments.
- Native service lifecycle commands execute `uv` and Docker directly instead
  of constructing shell-escaped command strings for `eval`. Background
  services inherit the legacy SIGHUP-ignore behavior, while PID files,
  per-service start locks, command arguments, environment loading, startup
  ordering, and text output remain compatible.
- Native service PID handling rejects PID 0. Bash accepted it, even though
  signaling PID 0 targets the CLI's entire process group rather than one
  managed service; treating a corrupt PID file as stopped avoids that unsafe
  side effect.
- Release uninstall continues to remove only the shared release directory and
  leaves the environment binding in metadata. Branch uninstall removes the
  environment-local checkout and its binding. Install and update write a
  binding only after download, extraction, synchronization, and any requested
  image build succeed; a multi-component install keeps work completed before a
  later component fails.

## Remote-data test policy

Commands that read remote refs use fixed git smart-HTTP advertisement fixtures
in the normal test suite. Characterization tests feed the same bytes to Bash
and Rust without network access, and failure cases use a deterministic failed
fetch. Live GitHub access is a manual integration check rather than a CI
requirement. This keeps snapshots stable while still allowing the production
transport to be checked before a remote-data slice is delivered.

`init` tests likewise use fixed archive bytes and a fixed UTC creation time.
They verify the requested branch through the exact download URL, then compare
the generated directory tree, metadata, and selected rendered files with Bash.

Install and update tests map each expected URL to a fixed response, allowing a
single invocation to resolve refs and then download an archive without network
access. Fake `uv` and Docker executables preserve and expose child-process
arguments and output order while creating only the completion markers the CLI
inspects.

GitHub archive extraction validates every entry before writing. Absolute or
escaping symbolic-link targets, hard links, special entries, duplicate paths,
and entries nested beneath an archive-provided symbolic link are rejected.
Extraction also refuses to traverse a symbolic link already present below the
target directory. A regression test verifies that a link followed by a nested
file cannot overwrite a file outside the target.

## Completion during the hybrid period

Shell completion must describe both migrated and legacy commands throughout
the hybrid period. It may initially remain a Bash route. Any later change in
its authoritative command model must be explicit and tested as part of the
slice that changes it.

## Delivery sequence

1. Introduce the Rust executable with every invocation delegated to Bash and
   an empty characterization test target wired to `insta`.
2. Choose a command slice and establish its reviewed Bash snapshots.
3. Implement and test that slice in Rust with fallback forbidden.
4. Switch its route to Rust.
5. Repeat until an audit against the Bash command surface confirms that every
   supported route is native.
6. Remove the fallback and `bin/oqtopus` on the migration branch, only through
   a separate, deliberate decision.
7. Add binary packaging, installation, and rollback on the migration branch.
8. Merge the migration branch into `main`.
9. Replace the hand-written argument parsing with `clap`. Behavior is kept
   where it matters; snapshot changes are reviewed one by one, and deviations
   in details that nothing depends on, such as malformed-argument errors and
   generated usage text, are accepted and recorded. The downstream consumer
   boundary is not relaxed by this step. As part of that refactoring, consolidate
   the repeated operation dispatch in `main.rs` and the shared engine handling
   and binding steps at the end of release and branch installation.

The historical characterization branch may be consulted if useful, but this
plan does not depend on reusing it.

## Planned slice order

The intended order starts with commands that matter to users but carry little
implementation risk, so that the routing, testing, and snapshot machinery is
established before harder slices. It may be reordered when a slice reveals a
better path.

1. Top-level `help`, `--help`, no arguments, and `version`, `--version`.
2. Read-only environment commands: `info` and `status` for `backend`,
   `cloud-local`, and `manager`, plus `backend device-status`.
3. `versions` for all three templates, establishing remote ref discovery and
   version ordering.
4. `init`.
5. `install`, `uninstall`, `update`, and `build`.
6. `start`, `stop`, and `restart`.
7. Completion moves to a Rust-owned command model, followed by fallback
   retirement and binary packaging.
8. After the compatible port is merged, the argument parser is refactored to
   `clap`, accepting reviewed changes in non-load-bearing details.

Slices 2 and 3 produce the output the Manager parses, so the first real slices
are also the first exercise of the downstream consumer boundary.

## Fallback retirement

The Bash implementation can be removed only after:

- every supported route is explicitly native;
- migrated-route tests cannot silently use fallback;
- compatibility decisions and intentional deviations have been recorded;
- the downstream consumer boundary is satisfied by the native implementations;
- completion covers the intended command surface; and
- Rust handles unknown commands and missing arguments without fallback,
  reproducing the Bash messages and exit status.

Before retirement, audit the Bash command surface against native routes and
tests, including supported commands, aliases, and help at every command level.
Record the audit with the retirement change. The native routing matches alone
do not prove completeness, because their catch-all still delegates to Bash.

Retirement happens on the migration branch before it merges. The order is:
remove the fallback and `bin/oqtopus`, then add binary packaging, installation,
and rollback, then merge into `main`. Packaging is therefore not a prerequisite
for retiring the fallback, but it is a prerequisite for the merge, so `main`
never lacks an installable CLI.

## Open migration decisions

- When completion moves from Bash to a Rust-owned command model.
- How the native executable is packaged, installed, and rolled back after
  fallback retirement and before the merge into `main`. Packaging must keep an `oqtopus` executable on `PATH`,
  because the Manager resolves it by name. Development keeps the current
  manifest-relative fallback lookup during migration. If hybrid distribution
  is introduced earlier, that change must settle how the legacy script is
  packaged and located.
- When structured (`--json`) output is introduced, and whether the
  human-readable output is frozen, kept as-is, or allowed to change at that
  point.
