# Command Reference

This page summarizes the user-facing OQTOPUS CLI commands.

## Top-Level Commands

```bash
oqtopus help
oqtopus --help
oqtopus version
oqtopus --version
oqtopus completion <bash|zsh|fish>
oqtopus init <env_name> --template backend
oqtopus backend <command>
```

## Environment Creation

```bash
oqtopus init <env_name> --template backend
```

Creates a local backend environment.

## Backend Information

```bash
oqtopus backend info
```

Prints backend environment metadata, including component version bindings and
expanded paths.

## Component Management

```bash
oqtopus backend versions <engine|tranqu|gateway>
oqtopus backend install <engine|tranqu|gateway> [<version>|branch:<branch>] [--skip-sse-build]
oqtopus backend install all [--skip-sse-build]
oqtopus backend build sse-runtime
oqtopus backend update <engine|tranqu|gateway>
oqtopus backend uninstall <engine|tranqu|gateway> <version>
```

`install all` installs the latest stable `engine`, `tranqu`, and `gateway`
releases independently.

`install` accepts either a release version tag (e.g., `v1.2.3`) or a branch
reference in the form `branch:<branch>` (e.g., `branch:develop`). Release
installs are stored in the shared installation root. Branch installs clone the
repository directly into `$ENV_ROOT/<component>/` and always re-clone on
repeated runs.

`install engine` and `install all` build the `sse_runtime` Docker image by
default. Use `--skip-sse-build` to defer only that Docker build, then run
`backend build sse-runtime` later to build the image for the engine version
bound in `.metadata`.

`versions` lists available stable versions from remote GitHub tags and does not
require a backend environment. When run inside a backend environment, it also
marks the current `.metadata` binding with `*`, locally available release
directories with `(installed)`, and any branch install with
`branch:<branch> (installed)`. Branch installs appear at the top of the list.

`uninstall` removes the selected local release directory without checking
whether another backend environment still references it. For branch installs,
pass `branch:<branch>` as the version argument: this removes
`$ENV_ROOT/<component>/` and also clears the binding from `.metadata`.

## Service Lifecycle

```bash
oqtopus backend start <core|sse_engine|mitigator|estimator|combiner|tranqu|gateway|all>
oqtopus backend start <core|sse_engine|mitigator|estimator|combiner|tranqu|gateway> --foreground
oqtopus backend stop <core|sse_engine|mitigator|estimator|combiner|tranqu|gateway|all>
oqtopus backend restart <core|sse_engine|mitigator|estimator|combiner|tranqu|gateway|all>
oqtopus backend status
```

`start`, `stop`, and `restart` require an explicit target. Use `all` to operate
on all managed services.

`--foreground` is available only for `start` with a single service target. It
keeps runtime stdout and stderr attached to the terminal for debugging.

## Device Status

```bash
oqtopus backend device-status show
oqtopus backend device-status active
oqtopus backend device-status inactive
oqtopus backend device-status maintenance
```

Valid device status values are `active`, `inactive`, and `maintenance`.

## Help

Help is available at the top level and for subcommands:

```bash
oqtopus help
oqtopus init help
oqtopus backend help
oqtopus backend install help
```

The same pattern applies to backend subcommands.
