# Starting And Stopping Cloud-Local Services

This page describes service lifecycle for `--template cloud-local`
environments. For backend service management, see
[Starting and Stopping Services](./backend-lifecycle.md).

## Managed Services

The managed services are:

| Service | Component | Management |
|---|---|---|
| `db` | `cloud` | Docker Compose (MySQL + MinIO) |
| `worker` | `cloud` | Process (PID file) |
| `user_signup` | `cloud` | Process (PID file) |
| `admin` | `cloud` | Process (PID file) |
| `provider` | `cloud` | Process (PID file) |
| `user` | `cloud` | Process (PID file) |

The `db` service starts a MySQL database and a MinIO object storage instance
via `docker compose`. All other services are uvicorn or Python processes
managed with PID files.

## Start All Services

```bash
oqtopus cloud-local start all
```

The start order is:

1. `db`
2. `worker`
3. `user_signup`
4. `admin`
5. `provider`
6. `user`

If a service is already running, `start` logs a skip message and continues.

## Start One Service

```bash
oqtopus cloud-local start user
```

For `db`, this runs `docker compose up -d db minio mc` and, if present,
`storage/init_storage.py` from the installed `cloud` component.

For other services, the process is started in the background and its PID
is written to `$ENV_ROOT/pids/<service>.pid`.

For debugging, start a single service in foreground mode:

```bash
oqtopus cloud-local start user --foreground
```

Foreground mode keeps the service attached to the terminal. Runtime stdout
and stderr are visible directly. `--foreground` cannot be used with `all`.

## Check Status

```bash
oqtopus cloud-local status
```

Example output:

```text
db: Running (my-cloud-db-1, my-cloud-minio-1, my-cloud-mc-1)
worker: Running (PID 12345)
user_signup: Running (PID 12346)
admin: Running (PID 12347)
provider: Running (PID 12348)
user: Running (PID 12349)
```

`db` shows the names of running Docker containers in the environment's Compose
project. Other services show their PID.

## Stop All Services

```bash
oqtopus cloud-local stop all
```

The stop order is the reverse of the start order:

1. `user`
2. `provider`
3. `admin`
4. `user_signup`
5. `worker`
6. `db`

`db` is stopped via `docker compose down`, which removes the containers.

## Stop One Service

```bash
oqtopus cloud-local stop user
```

Process services receive `TERM` and are waited up to 5 seconds. `db` is
stopped via `docker compose down`.

## Restart Services

Restart all managed services:

```bash
oqtopus cloud-local restart all
```

Restart one service:

```bash
oqtopus cloud-local restart user
```

## Process Output And Logs

By default, runtime stdout and stderr for process services are written to:

```text
$ENV_ROOT/logs/<service>/service.log
```

Use `--foreground` when you need to inspect runtime output directly while
debugging one service.

The `db` service output is managed by Docker and can be viewed with:

```bash
docker compose --project-name <env_name> logs
```
