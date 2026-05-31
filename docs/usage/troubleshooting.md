# Troubleshooting

This page lists common issues and how to recover from them.

## `oqtopus: command not found`

The install directory may not be on your `PATH`.

By default, `oqtopus` is installed to:

```text
~/.local/bin
```

Add that directory to your shell `PATH`, then restart your shell.

## `.metadata not found`

Backend commands must be run from inside a backend environment created by:

```bash
oqtopus init <env_name> --template backend
```

Change into the environment root before running backend commands:

```bash
cd <env_name>
oqtopus backend status
```

## Current Directory Does Not Match `env_root`

The current directory must match the `env_root` recorded in `.metadata`.

This can happen if the environment directory was moved after creation. Create a
new environment or move the directory back to its recorded path.

## Docker Is Required For Engine Install

Installing `engine` builds the `sse_runtime` Docker image.

If Docker is not installed or not running, commands such as these fail:

```bash
oqtopus backend install engine
oqtopus backend install all
```

Install and start Docker, then run the command again.

If you need to finish installation before Docker is available, use
`--skip-sse-build` and build the image later:

```bash
oqtopus backend install engine --skip-sse-build
oqtopus backend build sse-runtime
```

## `uv` Is Required

Backend components are synchronized and launched with `uv`.

Install `uv` before installing or starting backend components.

## `git` Is Required For Branch Install

Installing a component from a branch requires `git`.

If `git` is not installed, the following command fails:

```bash
oqtopus backend install engine branch:develop
```

Install `git`, then run the command again.

## Branch Name Not Found

If the specified branch does not exist in the remote repository, the clone
fails with an error from `git`.

Check that the branch name is correct, then run the command again with the
correct branch name:

```bash
oqtopus backend install engine branch:<correct-branch>
```

## Component Version Is Not Bound

If a service is started before its component is installed, `start` fails.

Install the required components:

```bash
oqtopus backend install all
```

Then start services again:

```bash
oqtopus backend start all
```

## Service Appears Already Running

If a PID file exists and the recorded process is alive, `start` skips that
service and logs a message such as:

```text
core is already running (PID 12345); skipping.
```

This means `start all` is safe to run at any time: services already running
are skipped and the rest are started normally.

If another `start` command is already launching the same service, `start` fails
before creating a duplicate process.

Check status:

```bash
oqtopus backend status
```

Stop the service if needed:

```bash
oqtopus backend stop <service>
```

## Stop Times Out

`stop` sends `TERM` and waits up to 5 seconds. If the process is still running
after that, `stop` exits with an error.

Inspect the process and stop it manually if required.

## Logs Are Missing

OQTOPUS CLI creates log directories, but it does not create log files.

Log files appear only after the backend applications write them according to the
`logging.yaml` files under `config/`.
