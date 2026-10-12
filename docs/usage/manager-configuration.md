# Manager Configuration

This page describes configuration for `--template manager` environments. For
backend configuration, see [Backend Configuration](./backend-configuration.md).
For cloud-local configuration, see [Cloud-Local Configuration](./cloud-local-configuration.md).

Configuration and asset files are created under `config/` and `assets/` when
you run:

```bash
oqtopus init <env_name> --template manager
```

The generated files are the same files [OQTOPUS Manager](https://github.com/oqtopus-team/oqtopus-manager)
itself ships as `config/config.yaml.example` and `config/logging.yaml`. Review
and update them as needed before starting the service.

## Configuration Directory

```text
config/
  config.yaml
  logging.yaml
assets/
  favicon.svg
  mv_bg.png
```

Unlike the backend and cloud-local templates, there is no `config/.env` and no
per-service subdirectory: manager has a single component and a single service,
and all of its settings live in `config/config.yaml`. `oqtopus manager start`
does not read a `config/.env` even if one is added by hand.

## `config/config.yaml`

`config/config.yaml` configures the manager web application: the bind address
and port, UI behavior, branding, authentication, and permissions. See the
comments in the generated file, and the
[OQTOPUS Manager documentation](https://oqtopus-manager.readthedocs.io/) for
the full list of supported options.

Notable settings:

- `server.port` — change this if you need to run more than one manager
  environment on the same host, since the default port (`38000`) is not
  templated per environment the way backend's `SSE_CONTAINER_IMAGE` is.
- `server.default_environment_base_path` and `server.environments_file` —
  these configure where OQTOPUS Manager stores the OQTOPUS environments *it*
  manages through its own web UI. This is a different concept from the
  `oqtopus-cli` environment created by `oqtopus init --template manager`. See
  [Manager Environment](./manager-environment.md) for the distinction.
- `auth.provider` — defaults to `none` for local use. Set to `header` to read
  identity from a proxy-injected JWT header when running behind a reverse
  proxy or identity-aware proxy.

## `config/logging.yaml`

`config/logging.yaml` configures Python logging for the manager process,
including the rotating file handler that writes to `logs/app.log` (relative to
`$ENV_ROOT`, since the manager service always runs with `$ENV_ROOT` as its
working directory).

## `environments/` and `config/environments.yaml`

OQTOPUS Manager creates and manages `environments/` and
`config/environments.yaml` itself at runtime; `oqtopus init` does not create
them. Do not confuse these with `oqtopus-cli`'s own `<env_name>/` environment
directory or `.metadata` file — they track a separate concept (the backend and
cloud-local environments that OQTOPUS Manager's web UI manages).

## Assets Are Not Auto-Updated

`oqtopus manager update` installs a newer manager release and updates the
`manager_version` binding in `.metadata`, but it does **not** modify
`config/config.yaml`, `config/logging.yaml`, `assets/`, or
`config/environments.yaml`. This is the same behavior as backend's
`config/.env` and cloud-local's `config/.env`: configuration and asset files
under an environment are never rewritten by `install`, `update`, or
`uninstall`.

If a newer manager release adds, renames, or removes configuration options or
expects different asset files, update `config/config.yaml`,
`config/logging.yaml`, and `assets/` manually. Compare against the new
release's `config/config.yaml.example`, `config/logging.yaml`, and `assets/`
directory in the [OQTOPUS Manager repository](https://github.com/oqtopus-team/oqtopus-manager).
