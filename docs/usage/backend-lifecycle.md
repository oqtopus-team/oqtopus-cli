# Starting And Stopping Services

This page describes service lifecycle for `--template backend` environments.
For cloud-local service management, see [Starting and Stopping Cloud-Local Services](./cloud-local-lifecycle.md).

OQTOPUS CLI can start, stop, restart, and inspect managed backend services from
inside a backend environment.

## Managed Services

The managed services are process targets for `start`, `stop`, `restart`, and
`status`. They are different from the installable component targets used by
`install`, `update`, and `uninstall`.

The installable `engine` component is a set of backend microservices. After
`engine` is installed, OQTOPUS CLI can manage these service targets from that
installed release:

- `core`
- `sse_engine`
- `mitigator`
- `estimator`
- `combiner`

`core` and `sse_engine` run from the installed `engine` `core` uv project.
`mitigator`, `estimator`, and `combiner` run from their matching engine uv
projects.

`tranqu` and `gateway` are installed as their own components and are also
managed as service targets:

- `tranqu`
- `gateway`

## Start All Services

```bash
oqtopus backend start all
```

The start order is:

1. `gateway`
2. `tranqu`
3. `mitigator`
4. `estimator`
5. `combiner`
6. `sse_engine`
7. `core`

If one service fails to start, the command stops and exits with an error.
Services already started by the same command are left running.

## Start One Service

```bash
oqtopus backend start gateway
```

If the service is already running, `start` fails instead of starting a duplicate
process.

OQTOPUS CLI also takes a short-lived start lock for each service while the
process is being launched. If two `start` commands for the same service run at
the same time, one of them fails before launching another process.

Started services are detached from the short-lived CLI process and continue
running after `oqtopus backend start` exits.

For debugging, start a single service in foreground mode:

```bash
oqtopus backend start gateway --foreground
```

Foreground mode keeps the service attached to the terminal, so runtime stdout
and stderr are visible. The command exits when the service process exits.
`--foreground` is supported only for one service at a time; it cannot be used
with `all`.

## Check Process Status

```bash
oqtopus backend status
```

Example output:

```text
core: Stopped
sse_engine: Stopped
mitigator: Running (PID 12345)
estimator: Stopped
combiner: Running (PID 12348)
tranqu: Stopped
gateway: Stopped
```

If a PID file exists but the process is no longer alive, the service is treated
as stopped.

## Stop All Services

```bash
oqtopus backend stop all
```

The stop order is the reverse of the start order:

1. `core`
2. `sse_engine`
3. `combiner`
4. `estimator`
5. `mitigator`
6. `tranqu`
7. `gateway`

If one service fails to stop, the command continues attempting to stop the
remaining services and exits with an error afterward.

## Stop One Service

```bash
oqtopus backend stop gateway
```

If no PID file exists, the service is treated as already stopped.

`stop` sends `TERM` and waits up to 5 seconds. It does not send `KILL`
automatically.

## Restart Services

Restart all managed services:

```bash
oqtopus backend restart all
```

Restart one service:

```bash
oqtopus backend restart gateway
```

For a single service, `restart` stops the service and starts it again. If
`stop` fails, the service is not started again.

For `restart all`, OQTOPUS CLI stops all services first. If every stop succeeds,
it starts all services again using the normal start order.

## Process Output And Logs

By default, runtime stdout and stderr are redirected to `/dev/null`.

Use `oqtopus backend start <service> --foreground` when you need to inspect
runtime stdout and stderr directly while debugging one service.

OQTOPUS CLI does not create application log files. Backend applications write
their own log files according to the `logging.yaml` files under `config/`.
