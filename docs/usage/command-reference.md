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
oqtopus backend install <engine|tranqu|gateway> [version]
oqtopus backend install all
oqtopus backend update <engine|tranqu|gateway>
oqtopus backend uninstall <engine|tranqu|gateway> <version>
```

`install all` installs the latest stable `engine`, `tranqu`, and `gateway`
releases independently.

`versions` lists available stable versions from remote GitHub tags and does not
require a backend environment. When run inside a backend environment, it also
marks the current `.metadata` binding with `*` and locally available release
directories with `(installed)`.

`uninstall` removes the selected local release directory without checking
whether another backend environment still references it.

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
