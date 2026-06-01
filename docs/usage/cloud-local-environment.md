# Cloud-Local Environment

A cloud-local environment is a local directory created by:

```bash
oqtopus init <env_name> --template cloud-local
```

Cloud-local commands must be run from the root of this environment.

## Environment Name

`env_name` is used as the local environment directory name, the Docker Compose
project name, and is recorded in `.metadata`.

Because it is used as a Docker Compose project name, `env_name` must be
Docker-safe.

Allowed pattern:

```text
^[a-z0-9][a-z0-9_.-]*$
```

Use lowercase letters, digits, `.`, `_`, or `-`, and start with a lowercase
letter or digit.

Examples:

- `my-cloud`
- `oqtopus_local`
- `cloud1`

## Directory Layout

```text
<env_name>/
  .metadata
  config/
    .env
  logs/
    db/
    worker/
    user/
    provider/
    admin/
    user_signup/
  pids/
```

## `.metadata`

`.metadata` records environment-specific information such as:

- the environment template (`cloud-local`);
- the environment name;
- the absolute environment path;
- the shared cloud-local installation root;
- the installed component versions bound to this environment.

Do not move an environment directory after creating it. Cloud-local commands
check that the current directory matches the `env_root` recorded in `.metadata`.

## `config/`

`config/.env` contains environment variables used when launching cloud-local
processes. These variables are applied only to the launched process environment;
they are not written to your global shell environment.

Default values include database connection settings, API ports, MinIO storage
configuration, and logging levels. Edit this file to customize the local
environment.

## `logs/`

`logs/` contains one directory for each managed service.

Runtime stdout and stderr for background-started services are written to
`logs/<service>/service.log`. Use `--foreground` to see output directly in the
terminal instead.

## `pids/`

`pids/` stores PID files for process-managed services (`worker`, `user_signup`,
`admin`, `provider`, `user`).

The `db` service is managed by Docker Compose and does not use a PID file.
Its status is checked via `docker compose ps`.
