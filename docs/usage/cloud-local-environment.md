# Cloud-Local Environment

A cloud-local environment is a local directory created by:

```bash
oqtopus init <env_name> --template cloud-local
```

The template is fetched from the `main` branch of `oqtopus-cli` by default.
Pass `--branch <branch>` to fetch it from another branch instead (mainly
useful when testing in-development templates):

```bash
oqtopus init <env_name> --template cloud-local --branch <branch>
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
check that the current directory matches the `environment_root` recorded in `.metadata`.

## `config/`

`config/.env` supplies settings to the cloud-local processes and the
database and storage command. These settings are scoped to the launched
commands. The template sets database connection settings, API ports, and
logging levels. `STORAGE_STACK` defaults to `seaweedfs` and can be set to
`minio`.

See [Cloud-Local Configuration](./cloud-local-configuration.md) for all
settings and instructions for updating an existing environment.

## `logs/`

`logs/` contains one directory for each managed service.

Runtime stdout and stderr for background-started services are written to
`logs/<service>/service.log`. Use `--foreground` to see output directly in the
terminal instead.

## `pids/`

`pids/` stores PID files for the process services `worker`, `user_signup`,
`admin`, `provider`, and `user`. `oqtopus-cloud-infra` manages MySQL and object
storage for the `db` service and reports their status. See
[Cloud-Local Lifecycle](./cloud-local-lifecycle.md) for the possible `db` states.
