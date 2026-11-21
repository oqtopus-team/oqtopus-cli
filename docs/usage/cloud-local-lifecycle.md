# Starting And Stopping Cloud-Local Services

This page describes service lifecycle for `--template cloud-local`
environments. For backend service management, see
[Starting and Stopping Services](./backend-lifecycle.md).

## Managed Services

The managed services are:

| Service | Component | Management |
|---|---|---|
| `db` | `cloud` | `oqtopus-cloud-infra` (MySQL + object storage) |
| `worker` | `cloud` | Process (PID file) |
| `user_signup` | `cloud` | Process (PID file) |
| `admin` | `cloud` | Process (PID file) |
| `provider` | `cloud` | Process (PID file) |
| `user` | `cloud` | Process (PID file) |

The `db` service manages MySQL and object storage through
`oqtopus-cloud-infra`. `STORAGE_STACK` selects SeaweedFS (the default) or MinIO.
All other services use the cloud component's API and worker entry points and
are managed with PID files.

Cloud v1.8.0 or later is required. If an installed cloud component does not
provide the required commands, update it with `oqtopus cloud-local update cloud`.
Existing environments also need the new settings in `config/.env`; see
[Cloud-Local Configuration](./cloud-local-configuration.md).

### Upgrading From A Cloud Before v1.8.0

This CLI cannot manage the `db` containers of a cloud before v1.8.0. With such a
cloud installed, `stop db` and `stop all` fail with the update instruction, and
`status` reports `db: Stopped` even while its containers are running.
Stop `db` before you upgrade the CLI:

1. With the CLI version that started the environment, run
   `oqtopus cloud-local stop all`.
2. Upgrade the CLI, then run `oqtopus cloud-local update cloud` and update
   `config/.env`.
3. Run `oqtopus cloud-local start all`.

Existing database and object storage data is not carried over. The first
`start db` after the upgrade creates and seeds a new database. With
`STORAGE_STACK=minio`, the existing MinIO volume is reused.

If the CLI is already upgraded and the old containers are still running, stop
them with Docker Compose. Use the old release's compose file and the
environment name as the project name:

```bash
docker compose --project-name <env-name> \
  -f <old-cloud-release>/backend/compose.yaml down
```

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

If a process service is already running, `start` logs a skip message and continues.
For `db`, the cloud infra command handles already-running containers and setup.

## Start One Service

```bash
oqtopus cloud-local start user
```

For `db`, this runs `oqtopus-cloud-infra start` from the installed cloud
component's `backend/` directory. The cloud command owns container startup,
readiness checks, database migrations, seeding, and storage initialization.
It resumes stopped containers when starting them again.

The database and process commands use `uv run --no-sync` with the environment
prepared by `install` or `update`. Starting services does not synchronize Python
dependencies or install dev dependencies. A missing `.venv` is rejected before
running the command.

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
db: Running (db, seaweedfs)
worker: Running (PID 12345)
user_signup: Running (PID 12346)
admin: Running (PID 12347)
provider: Running (PID 12348)
user: Running (PID 12349)
```

`db` covers the database and the object storage (`seaweedfs` or `minio`,
selected by `STORAGE_STACK`) as reported by `oqtopus-cloud-infra status`, and
shows one of the following. Other services show their PID.

| Output | Meaning |
| --- | --- |
| `db: Running (db, seaweedfs)` | Both are running. |
| `db: Partially running (db: running, seaweedfs: stopped)` | Only some are running. |
| `db: Stopped` | None is running, or the state cannot be determined. |

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

`db` is stopped with `oqtopus-cloud-infra stop`, which stops only the database
and object storage containers. Other containers, the network, and data volumes
are kept.

## Stop One Service

```bash
oqtopus cloud-local stop user
```

Process services receive `TERM` and are waited up to 5 seconds. `db` is
stopped with `oqtopus-cloud-infra stop`.

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

By default, runtime stdout and stderr for process services are written to
`$ENV_ROOT/logs/<service>/service.log`.

Use `--foreground` when you need to inspect runtime output directly while
debugging one service.

The `db` service output is managed by Docker and can be viewed with:

```bash
docker compose --project-name <env_name> logs
```
