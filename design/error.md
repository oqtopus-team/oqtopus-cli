# Rust CLI error handling

## Status and scope

> **Status:** proposed. No part of this design is implemented yet.

[Runtime architecture](runtime_architecture.md#deferred-work) defers the
cross-cutting error-handling redesign: retain underlying I/O, network, and
archive errors, add operation and path context, and centralize user-facing
formatting without breaking the Manager's exit and stream contract. This
document is that design. It defines the error model, the rules for writing an
error, how errors are rendered, and a plan in two stages placed around the
adoption of `clap`.

Module boundaries stay as the runtime architecture describes them, with one
new shared module, `error`. Compatibility changes made while implementing this
design are recorded with the other decided changes in
[Rust migration](rust_migration.md#decided-compatibility-changes).

The work is complete when:

- no function in `src/` returns `Result<_, String>` or `Result<_, ()>`;
- every failure caused by an I/O, HTTP, archive-validation, or child-process
  error keeps that error as its `source()`;
- one function in `text` renders every error, and `cli` maps every error to
  exit status 1; and
- tests of control flow assert error variants, while characterization
  snapshots keep pinning the rendered text.

## Current state

This section describes the code at `f6a985b`. Changes queued before stage 1,
such as running services with `uv run --no-sync` and managing the cloud-local
database through `oqtopus-cloud-infra`, replace some of the code named here,
including `retry_uv` and the `docker compose` steps. The examples illustrate
the problems; the rules in the rest of this document do not depend on them.

The port kept its error APIs deliberately simple: a failure is a `String`
formatted where it occurs, or `()` when the caller supplies the message. In
non-test code at `f6a985b`, `Result<_, String>` appears 87 times,
`Result<_, ()>` 14 times, and 61 places discard an error value or produce
`()`. That costs five things.

**Causes are lost.** A `()` error carries nothing, so the caller writes a
message that guesses at the cause.

- `remote::https_proxy` rejects an unusable `https_proxy` value with `()`. The
  user sees, for example, `failed to query GitHub tags API: <url>`, with no hint
  that a proxy variable is at fault.
- `operations::install_branch` reports `failed to download <component> from
  branch '<branch>'.` both when the download fails and when `archive` rejects
  an unsafe entry, and never names the rejected entry.
- `uv sync`, `docker build`, `docker compose`, and the `retry_uv` loop reduce
  "could not be executed" and "exited with status N" to one sentence.
  `service::start_process` reports a failed spawn as `The process exited
  immediately.`

**Kept causes are flattened.** Where a cause survives, it is interpolated into
a string (`format!("...: {error}")`). Callers can only compare text: the
start-lock tests assert `message.contains("already in progress")`, and
`metadata::INVALID_UTF8` is a string constant that two modules must emit
identically.

**Context is uneven.** Some messages name the object they failed on
(`failed to remove <path>: ...`); others do not (`failed to remove stale PID
file: ...`, `failed to create install root: ...`).

**Formatting is repeated.** `cli` spells out a `failed to write <result>`
message for every route, 28 times in all; `ensure_command` exists in both
`service` and `operations`; and three helpers produce `failed to write
progress: ...`.

**Domain wording sits in a mechanism.** `service::stop_process` takes a
`StopStyle` and chooses between the stop messages of the domains itself. Its
three variants hold two wordings, because backend and manager share one.
`service::start_process` likewise takes `manager_style: bool`, only to report
an immediate exit as `failed to start manager.` rather than `failed to start
'manager'.`. A typed error would have to carry the style to render its
message.

## Constraints

These come from `AGENTS.md`, the design records, and the Manager's source.
This design relaxes none of them.

- **Exit status and streams.** A failed command exits with status 1 and writes
  its report to stderr. Stdout receives nothing beyond progress that was
  already flushed. Usage output is a result, not an error: it stays on stdout
  with its own exit status. A foreground service still returns its child's
  status.
- **Message text reaches users through the Manager.** `oqtopus-manager` does
  not parse error text. For captured commands it turns a non-zero exit into an
  RFC 9457 problem whose `detail` is the whole stripped stderr, and logs it
  when a `status` check fails (`services/environment.py`). For streamed
  commands it merges stderr into the stdout line stream shown in its UI
  (`util/cli.py`). When this was written, no Manager code or test depended on
  a specific CLI error message; its tests mock stderr with arbitrary text.
  Nevertheless the text is what its users read, the Manager may come to
  depend on it, and this repository's
  [troubleshooting page](../docs/usage/troubleshooting.md) quotes messages.
  A change to existing text can therefore require a Manager change, and it is
  made only as a coordinated change: search the Manager's code, tests, and
  documentation for each changed message, change them in step when needed,
  update the quoted documentation here, and say so in the pull request.
- **Exit status 127 and line structure.** The Manager reads exit status 127 as
  "oqtopus is not installed", so the CLI never exits with 127, or with any
  status other than 1, for its own errors. Because streamed stderr is
  interleaved with progress and captured stderr is logged as one value, a
  report spans one line, except for the legacy multi-line hints listed under
  [Message text](#message-text): the renderer adds no line break, and a cause
  must not bring one in (see [Line structure](#line-structure)).
- **Snapshots are compatibility evidence.** Each change to rendered error text
  appears as a reviewed `.snap` diff with its reason, and intentional changes
  are recorded in the migration record.
- **Result data stays separate from rendering.** Errors are data. The words
  describing one failure live with its type, as its `Display`; the layout of a
  report (prefix, causes, newline, flush, stream) lives in `text`. A future
  `--json` renderer must be able to serialize the same values without parsing
  text.
- **Text is UTF-8.** Messages are `String`/`&str`. Paths are stored as
  `PathBuf` and rendered through `Path::display`. Invalid UTF-8 at a read
  boundary remains an explicit error.
- **Incremental output.** Progress is flushed before an error is reported, and
  the report flushes stderr, so the Manager's merged stream keeps the order in
  which things happened.
- **Clippy stays warning-free.** `clippy::result_large_err` warns when an `Err`
  type is larger than 128 bytes, its default `large-error-threshold`.

## Current Rust practice

The research behind the decisions, with sources listed under
[References](#references).

- **The std contract.** `std::error::Error` requires `Debug + Display`, and
  `source()` exposes the lower-level cause. Its documentation says messages
  "are typically concise lowercase sentences without trailing punctuation",
  and that a wrapping error returns the inner error from `source()` or renders
  it in `Display`, "but not both". `Error::provide` (generic member access,
  rust-lang/rust#99301), `Error::sources` (#58520), and `std::error::Report`
  (#90172) are still nightly-only, so this design depends on none of them.
- **API Guidelines, C-GOOD-ERR.** Error types implement `Error` and are
  `Send + Sync`, plus `'static` for downcasting. `()` is never an error type:
  it has no message, does not implement `Error`, and cannot be converted with
  `?`.
- **Reporting belongs to a reporter.** The Error Handling Project Group treats
  printing an error with its sources as a separate operation. Both
  `std::error::Report` and anyhow offer a single-line form that joins the chain
  with `": "` (`Report`'s default, anyhow's `{:#}`) and a multi-line
  `Caused by:` form.
- **Control flow versus reporting.** Palmieri chooses by consumer: enumerate
  variants where a caller reacts differently to each failure, and use an
  opaque error where every caller only reports it. "anyhow for applications,
  thiserror for libraries" is a heuristic, not a rule.
- **Modular errors.** Jewson places error types "near to their unit of
  fallibility": one type per operation or module, context as fields, variants
  named after the failing step rather than the library that reported it
  (`ReadFile`, not `Io`), and no `From<io::Error>` that treats every I/O error
  as the same failure.
- **Crates.** `thiserror` derives the `Display`, `Error`, and `From` impls one
  would write by hand and never appears in signatures. It renders
  `Path`/`PathBuf` fields through `display()`; version 2.0 added out-of-line
  formatting with `#[error(fmt = path)]` and requires a direct dependency.
  `anyhow` provides an opaque `anyhow::Error` with `.context()`, `snafu`
  generates context selectors, and `miette` and `color-eyre` render rich,
  colored diagnostics.
- **Third-party chains are imperfect.** `tempfile::PersistError` both prints
  and returns its `io::Error`. `ureq::Error` (3.x) prints its inner error and has
  no `source()`. Arti's reporter (`retry_error::fmt_error_with_sources`) joins
  with `": "` and skips a source whose text the previous message already
  contains. `tar` (0.4) reports an unpack failure as
  `` failed to unpack `<path>` `` with the I/O error as `source()`, so an entry
  name taken from a downloaded archive reaches the message text unescaped.
- **`fn main() -> Result<(), E>`** prints `Error: {err:?}`, the `Debug` form, and
  exits with status 1. It cannot reproduce this CLI's text, so `main` and `cli`
  keep rendering explicitly.

## Decisions

| Topic | Decision |
| --- | --- |
| Representation | Typed errors implementing `Error + Send + Sync + 'static`; no `String` or `()` errors |
| Derive | `thiserror` 2 as a direct dependency |
| `anyhow`, `snafu`, `miette`, `color-eyre` | Not adopted; see [Alternatives considered](#alternatives-considered) |
| Granularity | One error type per shared mechanism or domain, aggregated by one crate-level `Error` at the command boundary |
| Context | Fields of the variant, attached at the call site with `map_err`; no blanket `From<io::Error>` |
| Causes | Kept from stage 1 in a field named `cause`; `#[source]` alone decides whether the report shows it |
| `Display` | The error's own message only, never its source's text |
| Report | `text::write_error`: `Error: `, the message and every cause joined by `": "`, a newline, a flush; from stage 2, control characters in causes escaped |
| Exit status | Every `Error` maps to 1 |
| Message text | Stage 1 renders every message as today, byte for byte; stage 2 is the only change to text, coordinated with the Manager |
| Mechanism wording | Shared mechanisms return what happened; domains word it. `StopStyle` and `manager_style` are removed in stage 1 |
| Plan | Stage 1 (types) before `clap`, stage 2 (text) after it; see [Plan](#plan) |
| Backtraces, error codes | Not introduced |

### Error types

Errors live next to the code that fails and are aggregated once, at the
command boundary.

- Each module that can fail defines the error types for its own failures: the
  shared mechanisms (`args`, `remote`, `archive`, `environment`, `progress`,
  `service`, `versions`, `operations`), `init`, and the domains (`backend`,
  `cloud_local`, `manager`). A type is named after the operation
  (`FetchError`, `ExtractError`, `StartError`) and has one variant per distinct
  failure. A variant's name says what failed (`CreateInstallRoot`, `Spawn`),
  not which library reported it (`Io`).
- The new `error` module defines the crate-level `Error`: one
  `#[error(transparent)]` variant per module error type, plus the failure to
  write a final result. Command entry points return it and `cli` renders it.
  `transparent` forwards both `Display` and `source()`, so wrapping adds no text
  to a report.
- A shared mechanism returns its own types, never the aggregate. When it runs a
  domain callback, such as the command factory of `service::start_process`, it
  is generic over the callback's error. The domain's failure then passes
  through unchanged, and the mechanism does not depend on `error`. Nor does it
  hold domain wording; see
  [Wording of shared mechanisms](#wording-of-shared-mechanisms).

```rust
//! Failures returned by command entry points and rendered by `text`.

/// Every failure a command can report.
///
/// Each module variant forwards its message and source to the error it wraps.
#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Argument(#[from] ArgumentError),
    #[error(transparent)]
    Environment(#[from] EnvironmentError),
    #[error(transparent)]
    Operation(#[from] OperationError),
    #[error(transparent)]
    Start(#[from] StartError),
    // ... one variant per module error type.
    #[error("failed to write {what}")]
    WriteResult {
        what: &'static str,
        #[source]
        cause: io::Error,
    },
}
```

```rust
pub(crate) fn start_process<W, F, E>(/* ... */ make_command: F /* ... */) -> Result<i32, E>
where
    W: Write,
    F: FnOnce() -> Result<ServiceCommand, E>,
    E: From<StartError> + From<ProgressError>,
{
    // Lock, PID, and spawn failures are `StartError`s converted with `?`; a failure of
    // `make_command` is returned as the domain produced it.
}
```

Error types derive `Debug` because `Error` requires it. `Debug` output is
never shown to users.

### Context

Context is attached where the failing operation is known, as fields of the
variant that describes it.

- Construct the variant in `map_err` at the call site. Keep the object of the
  operation (path, URL, service, component, version) in a field even when the
  existing text does not print it; a structured renderer can use it later.
- Keep the cause. A variant built from a lower-level error stores it in a field
  named `cause`, even when the report does not show it yet. `thiserror` treats
  a field as the source only when it is named `source` or marked `#[source]` or
  `#[from]`, so an unmarked `cause` stays out of `source()` and out of the
  report, while tests can already match it. Marking it `#[source]` shows it.
  The attribute is therefore the only switch, and showing a cause later changes
  the type definition, not the call sites that build it.
- Do not implement `From<io::Error>`, `From<ureq::Error>`, or similar for a
  module error. Each I/O failure means something different, and `?` through a
  blanket conversion discards which operation failed. `#[from]` is used only to
  wrap a module error in the aggregate.
- Report absence as data. A lookup that can legitimately find nothing, such as
  a branch that does not exist or an empty tag list, returns `Option` or an
  empty collection, and the caller decides whether absence is an error. A
  transport failure stays a transport failure.
- Use `&'static str` for names fixed at compile time (service, component,
  template, program) and owned values for everything else.

The example shows the end state, after stage 2 has marked the download and
extraction causes `#[source]`. In stage 1 the same variants hold the same
`cause` fields without the attribute, and render today's text:

```rust
#[derive(Debug, thiserror::Error)]
pub(crate) enum OperationError {
    #[error("failed to download {component} {version} from {url}")]
    Download {
        component: &'static str,
        version: String,
        url: String,
        #[source]
        cause: FetchError,
    },
    #[error("failed to extract {component} {version}")]
    Extract {
        component: &'static str,
        version: String,
        #[source]
        cause: ExtractError,
    },
    // ...
}

fn download_release(
    component: Component,
    version: &str,
    target: &Path,
) -> Result<(), OperationError> {
    let url = format!(
        "https://github.com/{}/archive/refs/tags/{version}.tar.gz",
        component.repository
    );
    let archive = fetch_url(&url).map_err(|cause| OperationError::Download {
        component: component.name,
        version: version.to_owned(),
        url: url.clone(),
        cause,
    })?;
    extract_github_archive(&archive, target).map_err(|cause| OperationError::Extract {
        component: component.name,
        version: version.to_owned(),
        cause,
    })
}
```

The field is never named `source`: `thiserror` would treat that name as the
source without an attribute, and whether a cause is shown would no longer be
visible in one place.

### Message text

`Display` is the error's own message. It never contains the text of its
`source()`; the report appends causes. For the text itself:

- **Today's text is the baseline.** Until stage 2, a variant that replaces a
  current message renders the same bytes, including capitalization, a
  trailing period, and the multi-line hints of `.metadata`, `environment_root`,
  and environment-name errors. These hints are the only messages that span
  lines; new text contains no line break.
- **Outside text is escaped where it enters a message.** A variant that
  interpolates text read from outside the program, such as an archive entry
  name or data from a remote response, escapes its control characters, because
  the top-level message is written as is (see [Rendering](#rendering)). This
  applies to text introduced in stage 2; stage 1 reproduces today's bytes.
- **A message with a cause is one line without trailing punctuation**, because
  the report continues it with `": "`. Every message that ends with its cause
  today already has this form; a message that gains a cause in stage 2 loses
  its trailing period.
- **New text follows the std convention**: a concise lowercase phrase without
  trailing punctuation, quoting user-supplied names as existing messages do
  (`'core'`, `'develop'`).
- **No secrets.** A proxy URL may contain credentials, and `config/.env` holds
  credentials and keys. An error names the variable or file, never its value.
- **A change to existing text is a coordinated change**, made only in stage 2
  and under the Manager constraint above.

### Wording of shared mechanisms

A shared mechanism holds no domain wording. Where domains describe the same
event differently, the mechanism returns what happened as data, and each
domain words it. A mechanism's own messages are those every domain shows
unchanged, such as a failure to remove a PID file.

Two mechanisms word for domains today, both in `service`.

`start_process` loses its `manager_style` parameter. Its
`StartError::ExitedImmediately { service }` renders the wording backend and
cloud-local share, `failed to start '<service>'. The process exited
immediately.`. The manager replaces that variant with its own
`ManagerError::ExitedImmediately`, which keeps today's unquoted
`failed to start manager.`.

`stop_process` loses its `StopStyle` parameter and returns the outcome:

```rust
/// What stopping a process-backed service found.
pub(crate) enum StopOutcome {
    NotRunning,
    Stopped { pid: u32 },
}

pub(crate) fn stop_process(root: &Path, service: &str) -> Result<StopOutcome, StopError>
```

- The domain writes the progress line for each outcome: `core: Stopped` and
  `Stopped core` for backend and manager, `user is not running.` and
  `Stopped user (PID 123)` for cloud-local.
- `StopError` has one variant per failure: `Signal { pid, cause }` when TERM
  cannot be sent, `Timeout { pid }`, and `RemoveStalePidFile` and
  `RemovePidFile { path, cause }`. The PID-file variants read the same for
  every domain and are wrapped transparently. The domain maps `Signal` and
  `Timeout` to its own variants, such as `BackendError::StopTimeout { service,
  pid }`, which render its wording and keep the `StopError` as their `cause`.
- Backend and manager each spell their wording. The strings are equal today
  because the commands share a style, not because one depends on the other.

### Rendering

`text::write_error` is the only code that formats a report. Domain code that
reports a failure and continues, as `stop all` and `restart all` do, calls it
as well.

```rust
/// Writes `error` and every cause as one `Error:` report, then flushes.
///
/// Causes follow the message after `": "`, and the only line break written is the final newline.
/// Control characters in a cause are escaped: its text may come from outside the program, and must
/// neither split the report nor drive the terminal.
pub(crate) fn write_error(out: &mut impl Write, error: &dyn std::error::Error) -> io::Result<()> {
    write!(out, "Error: {error}")?;
    let mut cause = error.source();
    while let Some(source) = cause {
        write!(out, ": {}", escape_control(&source.to_string()))?;
        cause = source.source();
    }
    writeln!(out)?;
    out.flush()
}
```

This is the single-line form of `std::error::Report` and of anyhow's `{:#}`.
It reproduces every existing `<context>: <cause>` message byte for byte, which
is what lets stage 1 change no output, and needs no dependency. Stage 1 writes
causes without `escape_control`; stage 2 adds it together with the causes it
shows.

**Every cause is printed.** The renderer does not skip a cause whose text its
parent already contains, as Arti's reporter does. Matching by substring would
hide an unrelated cause that happens to occur in its parent's message
(`failed to update database` and a cause `database`), and would make stage 1's
byte-for-byte reproduction depend on the text of causes. A third-party error
that both prints and returns its source is handled where it is wrapped: the
variant stores the inner error, such as `PersistError::error`, as its cause.
No current dependency needs this. `tempfile` is used only for `tempdir`,
`ureq::Error` has no `source()`, and `tar`'s `TarError` prints only its
description and returns the I/O error as its source.

#### Line structure

The renderer adds no line break before the final newline, but the text it
writes can still contain one. Two kinds of text can:

- **The legacy multi-line hints**, the only messages that span lines (see
  [Message text](#message-text)). The top-level message is written as is so
  that they stay intact.
- **Text from outside the program.** A third-party cause can embed it, as `tar`
  embeds an entry name from a downloaded archive, and this program cannot
  control that `Display`. The renderer therefore escapes control characters,
  line feeds included, in every cause, from stage 2. A variant that puts outside
  text into its own message escapes it itself. Escaping replaces control
  characters only, so text a variant has escaped passes the renderer unchanged.

Stage 1 marks as a source only the causes that today's text already ends with.
They come from std's file-system and clock calls, whose messages contain no
outside text, so escaping is first needed when stage 2 shows network, archive,
and child-process causes.

### Exit status

| Situation | stdout | stderr | Status |
| --- | --- | --- | --- |
| Success | result | nothing | 0 |
| Usage requested or invalid form | usage text | nothing | 0 or 1, as today |
| `Err(Error)` from a command | progress already flushed | one report | 1 |
| Some services fail in `stop all` or `restart all` | progress | one report per failure | 1 |
| Foreground service | progress | the child's own output | the child's status |
| Command-line arguments are not UTF-8 | nothing | one report | 1 |

Only `cli` maps a command's `Err` to an exit status, and the status is
always 1. `main` does the same for arguments that are not UTF-8, through the
same `write_error` and an `ArgumentError` variant. No error carries its own
exit status: status 1 is the contract for every failure, and the Manager
reserves 127. The usage row is revisited by `clap`; see
[Ordering with `clap`](#ordering-with-clap).

### Size

Every command returns `Result<_, Error>`, so the aggregate's size matters, and
`clippy::result_large_err` limits it to 128 bytes. The limit is close: on
64-bit targets `ureq::Error` alone is 64 bytes, so a download variant holding a
component name, a version, a URL, and an unboxed `FetchError` measures exactly
128 bytes. Box a large third-party error where it is wrapped
(`Transport { cause: Box<ureq::Error> }` brings `FetchError` to 24 bytes and
that variant to 88), use `&'static str` for fixed names, and fix the size instead of
silencing the lint with `#[allow]`. Stage 1 already holds every cause, so the
limit applies from stage 1, not only once stage 2 shows the causes.

### Panics and ignored errors

No code outside tests calls `unwrap` or `expect` today, and failures that can
happen at run time (I/O, network, child processes, user input, environment)
stay `Err`. A panic exits with status 101 and prints a Rust diagnostic, both
outside the contract. `expect` is reserved for internal invariants and states
the invariant.

An error is ignored deliberately only in two cases, and a comment says which:

- **Absence by contract.** `running_pid`, `load_env_exports`,
  `migrate_metadata_keys`, `installed_versions`, and the container-name lookup
  of `cloud-local status` treat a failure as "absent" or "not running", as
  recorded compatibility behavior.
- **Best-effort cleanup.** Removing a PID file or lock directory after a
  failure, or on drop. Cleanup never replaces the error that caused it.

Writing the report itself is the one place where a failed write is dropped:
when stderr is unusable, nothing is left to report it to.

### Dependency

`thiserror = "2"` becomes a direct dependency. It generates exactly the
`Display`, `Error`, and `From` impls that would otherwise be written by hand,
and it never appears in a signature, so reverting to hand-written impls would
change no caller. `thiserror` 2.0.21 and its proc-macro dependencies are already
in `Cargo.lock` through `rustls-platform-verifier` on Android, so the lockfile
gains no new crate versions; only the root package's dependency list changes,
and it is committed because CI builds with `--locked`. The CI targets, which
compile no proc-macro crate today, start compiling `syn`, `quote`, and
`proc-macro2`; this adds to a clean build's time and nothing to the binary.

## Module inventory

Variant names are representative; the implementing change settles them. The
inventory reflects the modules at `f6a985b`, and stage 1 revises it first. The
last column lists what stage 2 changes; stage 1 changes no text anywhere.

| Module | Error types | Representative variants | Stage 2 |
| --- | --- | --- | --- |
| `args` | `ArgumentError` | `NotUtf8`, `UnknownCommand`, `UnexpectedArguments`, `UnknownService`, `UnknownComponent`, `UnknownOption`, `UnsupportedShell` | Mostly replaced by `clap` before stage 2 |
| `remote` | `FetchError`, `ProxyError` | `Proxy { variable, reason }`, `Transport { cause: Box<ureq::Error> }`, `ReadBody { cause }` | Reported as a cause |
| `archive` | `ExtractError` | `Decode`, `UnsafeEntry { path, reason }`, `TargetIsSymlink`, `Write { path, cause }` | Reported as a cause, entry names escaped |
| `environment`, `metadata` | `EnvironmentError` | `NotFound { template }`, `InvalidUtf8`, `MissingKey { key }`, `TemplateMismatch`, `RootMismatch`, `CurrentDirectory` | Candidate: unreadable `.metadata` |
| `progress` | `ProgressError` | `Write { cause }` | None |
| `service` | `StartError`, `StopError`, `ChildError` | `Busy { service, owner }`, `Lock { path, cause }`, `Spawn { service, cause }`, `ExitedImmediately`, `Signal { pid, cause }`, `Timeout { pid }`, `RemovePidFile { path, cause }` | Spawn failure reported with its cause |
| `versions` | `VersionsError` | `QueryTags { url, cause }`, `NoTags { url }`, `NoStableVersions { component }` | Cause of a failed tag query |
| `operations` | `OperationError` | `Download`, `Extract`, `Sync`, `BuildImage`, `CreateInstallRoot`, `Remove { path, cause }`, `NotInstalled { path }`, `UpdateMetadata` | Causes appended; download and extraction told apart |
| `init` | `InitError` | `InvalidName`, `TargetExists`, `UnknownTemplate`, `Download`, `Extract`, `TemplateMissing`, `UnsafeTemplateLink`, `Copy`, `HomeNotSet` | Causes of download, extraction, and link checks |
| `backend`, `cloud_local`, `manager` | `BackendError`, `CloudLocalError`, `ManagerError` | missing binding, release directory not found, device-status file, argument conflicts, database start and stop steps, `StopSignal`, `StopTimeout`, `ManagerError::ExitedImmediately` | Causes of child-process steps (uv, Docker, `oqtopus-cloud-infra`); candidate: one start and stop wording |
| `error` | `Error` | one transparent variant per type above, `WriteResult { what, cause }` | None |

`ChildError` describes one external program that could not run to a
successful exit: `NotFound { program }`, which the single `ensure_command`
returns with today's `'uv' is required but was not found on PATH.` text;
`Spawn { program, cause }`; or `Status { program, status }`, rendered with
`ExitStatus`'s own text (`exit status: 1`, `signal: 15 (SIGTERM)`). It lives in
`service`, because child execution is already that module's responsibility,
and `operations` and the domains use it for `uv` and Docker.

## Plan

The work has two stages. Stage 1 changes the types and nothing a user or the
Manager can observe; stage 2 changes text and nothing else. `clap` goes
between them.

### Stage 1: typed errors, identical output

- Start by recounting the error sites and revising the
  [Module inventory](#module-inventory) against the code of the day. The
  counts and examples in this document describe `f6a985b`.
- Add `thiserror`, the `error` module, `text::write_error`, and the module
  error types, and replace every `String` and `()` error with them, including
  the duplicated `ensure_command`, progress-line helpers, and `failed to write
  ...` closures.
- Every variant renders today's bytes and keeps the error it was built from as
  its `cause`. The cause is marked `#[source]` only where today's text already
  ends with it (`failed to create install root: <cause>`), which the renderer
  reproduces exactly. A cause that today's text omits is typed where it arises
  (`FetchError`, `ExtractError`, `ChildError`), passed by the caller, and held
  unmarked, so it is kept and tested but not shown. Where today's code throws a
  cause away, keeping it is part of stage 1: a retry loop, such as `retry_uv`
  at `f6a985b`, discards every attempt's result and must keep the last one.
  Failures that share one
  message today, such as a branch download and its extraction, or a failed
  spawn and an immediate exit, become separate variants with the same text.
- Remove `StopStyle` and `manager_style`. `stop_process` returns
  `StopOutcome` and `StopError`, and the backend, cloud-local, and manager stop
  paths write today's lines and messages from them; the manager words an
  immediate exit itself (see
  [Wording of shared mechanisms](#wording-of-shared-mechanisms)).
- Argument errors get only the variants needed to reproduce today's messages,
  because `clap` replaces most of them.
- The characterization suite passes without a single snapshot change. That is
  necessary but not sufficient evidence that stage 1 changed no output. A
  snapshot records the final stdout and stderr of the cases it runs, each
  separately. It does not show the order of progress and error output, whether
  `stop all` continues after a failure, how often a retry loop retries, or any
  error path that no case reaches. Stage 1 therefore adds the behavior tests
  listed under [Testing](#testing) for the paths it rewrites. The unchanged
  suite and those tests together are why it needs no Manager coordination.
- Stage 1 lands as one pull request if it stays reviewable. If it is split,
  introduce `Error` first with a temporary `#[error("{0}")] Legacy(String)`
  variant and `From<String>`, replace it module by module, and remove it
  before stage 1 ends; every pull request keeps the output identical.

### Ordering with `clap`

`clap` is the next planned change to argument handling. It replaces most
argument errors (unknown commands, options, and values; unexpected arguments)
with `clap::Error`, and it must decide how those are presented: `clap` reports
an error on stderr as `error: ...` followed by usage and exits with status 2,
while the CLI today prints usage on stdout or `Error: ...` on stderr, and exits
with status 1.

- **Stage 1 goes before `clap`.** It changes no output, so it does not
  constrain `clap`'s presentation decisions or involve the Manager. It gives the
  `clap` change a typed target: domain validations such as `--foreground` with
  `all`, `--skip-sse-build` outside `engine`, `install all` with a version, and
  an empty `branch:` are already typed variants, whether they stay in the
  domains or move into `clap`, and `cli` is already the one place that maps a
  failure to an exit status. The cost is the handful of argument variants that
  `clap` later deletes.
- **Stage 2 goes after `clap`.** Both change user-visible text. Ordered this
  way, one style decision covers argument errors and runtime errors together
  (prefix, capitalization, trailing periods), and the Manager is checked and,
  if needed, updated once instead of twice. Ideally `clap` and stage 2 ship in
  the same release.

If `clap` is postponed for long, stage 2 may go first. Its style decision is
then made without `clap`, and the `clap` change must render its errors to
match it.

### Stage 2: text changes

Stage 2 is the only change to text, and it changes only text: `#[error(...)]`
strings, `#[source]` attributes on causes that stage 1 already keeps, and
escaping in the renderer. It touches no call site that builds an error and no
control flow, so its review is about wording and its consumers.

- Mark as `#[source]` the causes that stage 1 kept but did not show: network
  and proxy failures, archive rejections, and child-process failures,
  including the failed spawn and the last failed attempt of a retry loop.
- Escape control characters in causes, and in outside text that a variant puts
  into its own message (see [Line structure](#line-structure)).
- Drop the trailing period of each message that gains a cause, and give the
  variants that stage 1 separated their own text.
- If decided with `clap`, align the remaining legacy text with the style chosen
  for `clap`'s errors.
- Candidate: report an unreadable `.metadata` with its real cause, such as
  permission denied, instead of the legacy `invalid .metadata: missing
  template.`. Recommendation: do it; no consumer depends on the misleading
  text.
- Candidate: give every domain one start and one stop wording, by changing
  the strings of the domains' stop lines and variants, and of
  `ManagerError::ExitedImmediately`. Folding those variants into the messages
  of `StartError` and `StopError` afterwards changes code, not text, and is not
  part of stage 2. `backend stop` output, including `core: Stopped`, is quoted
  in [Backend lifecycle](../docs/usage/backend-lifecycle.md).

Each change follows the Manager constraint: the Manager's code, tests, and
documentation are checked for every changed message and updated in step when
needed, [troubleshooting](../docs/usage/troubleshooting.md) and any other page
quoting a message is updated, every snapshot change is reviewed, and the
decisions are recorded in the migration record.

Representative reports before and after stage 2, with wording settled in the
implementing change:

```text
# uv sync exits with status 1
before: Error: failed to synchronize manager v2.0.0 with uv.
after:  Error: failed to synchronize manager v2.0.0 with uv: uv failed with exit status: 1

# the branch archive contains a link that leaves the archive
before: Error: failed to download engine from branch 'develop'.
after:  Error: failed to extract engine from branch 'develop': archive entry 'sse_runtime/link' points outside the archive

# https_proxy names an unsupported scheme
before: Error: failed to query GitHub tags API: https://github.com/oqtopus-team/oqtopus-engine.git/info/refs?service=git-upload-pack
after:  Error: failed to query GitHub tags API: https://github.com/oqtopus-team/oqtopus-engine.git/info/refs?service=git-upload-pack: https_proxy does not name a usable proxy: unsupported scheme

# the uv executable cannot be executed when a service starts
before: Error: failed to start 'core'. The process exited immediately.
after:  Error: failed to start 'core': failed to run uv: Permission denied (os error 13)

# unchanged: the cause is already appended today
Error: failed to create install root: Permission denied (os error 13)
```

## Testing

- **Variants, not text, for control flow.** Unit tests assert variants, for
  example `assert!(matches!(contender, Err(StartError::Busy { owner: None, .. })))`
  in place of `message.contains("already in progress")`. Error types need not
  implement `PartialEq`, which `io::Error` does not. A cause that stage 1 keeps
  but does not show is asserted the same way, for example
  `matches!(error, OperationError::Sync { cause: ChildError::Status { .. }, .. })`,
  so its retention is tested before stage 2 shows it. `stop_process` is
  tested through its `StopOutcome` and `StopError` values, and each domain's
  stop wording through the CLI.
- **Renderer.** `text` unit tests cover a leaf error, a two-level chain, a
  transparent wrapper without duplicated text, a cause whose text also occurs
  in its parent's message and is still printed, a multi-line legacy message
  rendered unchanged, and the flush. From stage 2 they also cover a cause
  containing a line feed and an escape character, such as an archive entry
  name, rendered on one line with both escaped.
- **Behavior on rewritten paths.** Stage 1 adds focused tests for what
  unchanged snapshots cannot show (see
  [Stage 1](#stage-1-typed-errors-identical-output)):
  - ordering of progress and error output, through a harness helper that gives
    the child one pipe as both stdout and stderr, as the Manager does for
    streamed commands;
  - `stop all` and `restart all` continuing after the first failed service,
    writing one report per failure, and exiting with status 1;
  - for each retry loop, its number of attempts and its warning lines, with
    the delay between attempts injectable so the test does not sleep.
- **Characterization.** Snapshots keep pinning rendered text, and focused
  assertions keep pinning exit status and empty stdout. Stage 1 produces no
  `.snap.new` at all. In stage 2, every snapshot change is reviewed
  individually, and its text change is listed in the pull request together
  with the result of the Manager check.
- **Test hooks return production errors.** `test_hooks::http_fixture` returns a
  `FetchError`. A missing fixture is reported as an HTTP 404 response, the
  failure a missing tag or branch produces in production, so that once stage 2
  reports fetch failures, snapshots show what a user would see.
- **New error paths get tests**, as every change does: proxy misconfiguration
  and archive rejection reasons in unit tests in stage 1, and a spawn failure,
  a failing `uv sync`, and an archive entry name with a control character
  through the CLI in stage 2.

## Alternatives considered

- **Keep `String` and improve the messages.** Fixes wording but not identity:
  control flow still compares text, causes are still flattened, and a JSON
  renderer would have to parse messages.
- **`anyhow` throughout.** The least boilerplate, with chains for free. But
  messages would again be strings built inside command logic, against the
  separation of result data and rendering; tests and a future JSON renderer
  would have nothing stable to match; and what it would save, rendering a
  chain, is the dozen lines of `write_error`.
- **One flat crate-wide enum.** Easy to find, but every module would depend on
  one type, unrelated failures would share it, and generic `Io(io::Error)`
  variants would appear that no longer say which operation failed.
- **`snafu`.** Its context selectors suit per-module errors, but they add
  generated types and macro surface. Explicit `map_err` closures with
  `thiserror` are the common idiom and review well.
- **`miette` or `color-eyre`.** Colored, multi-section diagnostics conflict with
  the plain, stable text the Manager forwards.
- **Hand-written impls.** Equivalent output without a proc-macro dependency,
  but three impls per type. Because `thiserror` never appears in signatures,
  this choice can be reversed later without touching callers.
- **Multi-line `Caused by:` reports.** These would change every existing
  `<context>: <cause>` message and split one failure across several lines of
  the Manager's interleaved stream and logs.
- **Skipping a cause whose text its parent contains**, as Arti's reporter does.
  It hides unrelated causes that happen to match and makes stage 1's
  byte-for-byte reproduction depend on the text of causes; see
  [Rendering](#rendering).
- **Attaching hidden causes at the call sites in stage 2.** Stage 1 would type
  a cause where it arises but leave callers discarding it. Stage 2 would then
  rewrite every such `map_err` and restructure code such as a retry loop, which
  is a change to control flow inside what should be a review of wording.
  Keeping the cause unmarked from stage 1 leaves stage 2 one attribute per
  variant.
- **Keep `StopStyle` and `manager_style`, and store them in the errors.**
  Stage 1 would change less
  code, but the error would carry a rendering choice instead of data, and the
  shared mechanism would keep each domain's wording.
- **Exit status per error category, as in `sysexits.h`.** The contract is
  status 1 and the Manager reserves 127; changing either is a coordinated
  change outside this design.
- **Types and text in one change.** The snapshot diff would be the only
  evidence for both a large mechanical refactoring and a Manager-coordinated
  wording change, and neither could land without the other. Separate stages
  let stage 1 prove itself by an unchanged suite.
- **`clap` before stage 1.** Viable, and it would spare the few argument
  variants `clap` deletes. But the validations written during the `clap`
  change would be strings to retype later, and `clap` would settle error
  presentation without the error model. Stage 2 after `clap` holds either way.

## Open questions

- **Stable error codes.** A `--json` renderer will need stable machine-readable
  identifiers. They are defined explicitly with its schema, not derived from
  Rust type names, which refactoring changes.

## References

- [`std::error::Error`](https://doc.rust-lang.org/std/error/trait.Error.html)
  and [`std::error::Report`](https://doc.rust-lang.org/std/error/struct.Report.html)
  (unstable, rust-lang/rust#90172)
- [`std::process::Termination`](https://doc.rust-lang.org/std/process/trait.Termination.html)
- [Rust API Guidelines, C-GOOD-ERR](https://rust-lang.github.io/api-guidelines/interoperability.html#c-good-err)
- [What the Error Handling Project Group is Working Towards](https://blog.rust-lang.org/inside-rust/2021/07/01/What-the-error-handling-project-group-is-working-towards/)
- [thiserror](https://github.com/dtolnay/thiserror) and its
  [2.0.0 release notes](https://github.com/dtolnay/thiserror/releases/tag/2.0.0)
- [anyhow](https://github.com/dtolnay/anyhow), including the display
  representations of [`anyhow::Error`](https://docs.rs/anyhow/latest/anyhow/struct.Error.html)
- [`clap::error::Error`](https://docs.rs/clap/latest/clap/error/struct.Error.html),
  whose `exit` reports errors on stderr with status 2
- Sabrina Jewson, [Modular Errors in Rust](https://sabrinajewson.org/blog/errors)
- Luca Palmieri, [Error Handling In Rust - A Deep Dive](https://www.lpalmieri.com/posts/error-handling-rust/)
- [`retry_error::fmt_error_with_sources`](https://docs.rs/retry-error/latest/retry_error/fn.fmt_error_with_sources.html)
  (Arti)
- `oqtopus-manager`:
  [`util/cli.py`](https://github.com/oqtopus-team/oqtopus-manager/blob/main/src/oqtopus_manager/util/cli.py)
  and [`services/environment.py`](https://github.com/oqtopus-team/oqtopus-manager/blob/main/src/oqtopus_manager/services/environment.py)
