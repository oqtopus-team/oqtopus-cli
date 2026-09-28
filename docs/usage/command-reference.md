# Command Reference

This page summarizes the user-facing OQTOPUS CLI commands.

Arguments must be valid UTF-8. Invalid arguments produce an `Error: ...` message
on stderr and exit status 1. An empty command word shows help, including after
`backend`, `cloud-local`, or `manager`. For `backend` and `cloud-local`, an empty
`start` or `install` target shows command usage and exits with status 1.

## Top-Level Commands

```bash
oqtopus help
oqtopus --help
oqtopus version
oqtopus --version
oqtopus completion <bash|zsh|fish>
oqtopus init <env_name> --template <backend|cloud-local|manager> [--branch <branch>]
oqtopus backend <command>
oqtopus cloud-local <command>
oqtopus manager <command>
```

## Environment Creation

```bash
oqtopus init <env_name> --template cloud-local
oqtopus init <env_name> --template backend
oqtopus init <env_name> --template backend --branch <branch>
oqtopus init <env_name> --template manager
```

Creates a local environment from the specified template. Template symbolic links
must resolve within the copied template; escaping links and link cycles are
rejected before copying or rendering files.

By default, the template is fetched from the `main` branch of `oqtopus-cli`.
Pass `--branch <branch>` to fetch it from a different branch instead — mainly
useful for testing in-development templates.

## Cloud-Local Component Management

`install`, `uninstall`, and `update` require `environment_name` (or the legacy
`env_name`) in `.metadata`, as well as `install_root`.

When `status` finds multiple containers for a Compose service, it reports the
first name so each service remains on a single output line.

```bash
oqtopus cloud-local versions <cloud|frontend|admin>
oqtopus cloud-local install <cloud|frontend|admin> [<version>|branch:<branch>]
oqtopus cloud-local install all
oqtopus cloud-local update <cloud|frontend|admin>
oqtopus cloud-local uninstall <cloud|frontend|admin> <version>
oqtopus cloud-local uninstall <cloud|frontend|admin> branch:<branch>
```

`install all` installs the latest stable `cloud`, `frontend`, and `admin`
releases independently.

`install` accepts either a release version tag (e.g., `v1.2.3`) or a branch
reference in the form `branch:<branch>` (e.g., `branch:develop`). Branch
installs clone the repository into `$ENV_ROOT/<component>/`.

`uninstall` with `branch:<branch>` removes `$ENV_ROOT/<component>/` and clears
the binding from `.metadata`.

## Cloud-Local Service Lifecycle

```bash
oqtopus cloud-local start <db|user|provider|admin|user_signup|worker|all>
oqtopus cloud-local start <db|user|provider|admin|user_signup|worker> --foreground
oqtopus cloud-local stop <db|user|provider|admin|user_signup|worker|all>
oqtopus cloud-local restart <db|user|provider|admin|user_signup|worker|all>
oqtopus cloud-local status
```

The `db` service is managed via Docker Compose. All other services are managed
as local processes with PID files.

`start all` starts services in the order: `db`, `worker`, `user_signup`,
`admin`, `provider`, `user`. `stop all` uses the reverse order.

`--foreground` is available only for `start` with a single service target.
Runtime stdout and stderr are visible in the terminal; logs are written to
`$ENV_ROOT/logs/<service>/service.log` for background starts.

`status` shows Docker container names for `db` and PIDs for other services.

## Cloud-Local Environment Info

```bash
oqtopus cloud-local info
```

Prints cloud-local environment metadata.

## Backend Information

```bash
oqtopus backend info
```

Prints backend environment metadata, including component version bindings and
expanded paths.

## Backend Component Management

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

## Backend Service Lifecycle

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

## Backend Device Status

```bash
oqtopus backend device-status show
oqtopus backend device-status active
oqtopus backend device-status inactive
oqtopus backend device-status maintenance
```

Valid device status values are `active`, `inactive`, and `maintenance`.

## Manager Information

```bash
oqtopus manager info
```

Prints manager environment metadata.

## Manager Component Management

```bash
oqtopus manager versions
oqtopus manager install [<version>|branch:<branch>]
oqtopus manager update
oqtopus manager uninstall <version>
oqtopus manager uninstall branch:<branch>
```

Unlike `oqtopus backend` and `oqtopus cloud-local`, these commands do not take
a component name argument: manager has a single installable component.

`install` accepts either a release version tag (e.g., `v0.1.0`) or a branch
reference in the form `branch:<branch>` (e.g., `branch:develop`). Release
installs are stored in the shared installation root. Branch installs clone the
repository directly into `$ENV_ROOT/manager` and always re-clone on repeated
runs.

`versions` lists available stable versions from remote GitHub tags and does
not require a manager environment. When run inside a manager environment, it
also marks the current `.metadata` binding with `*`, locally available release
directories with `(installed)`, and any branch install with
`branch:<branch> (installed)`.

`uninstall` removes the selected local release directory without checking
whether another manager environment still references it. For a branch
install, pass `branch:<branch>` as the version argument: this removes
`$ENV_ROOT/manager` and also clears the binding from `.metadata`.

## Manager Service Lifecycle

```bash
oqtopus manager start
oqtopus manager start --foreground
oqtopus manager stop
oqtopus manager restart
oqtopus manager status
```

Unlike `oqtopus backend` and `oqtopus cloud-local`, these commands do not take
a service name argument or an `all` target: manager has a single managed
service.

`--foreground` keeps runtime stdout and stderr attached to the terminal for
debugging.

## Help

Help is available at the top level and for subcommands:

```bash
oqtopus help
oqtopus init help
oqtopus backend help
oqtopus backend install help
```

The same pattern applies to backend subcommands.
