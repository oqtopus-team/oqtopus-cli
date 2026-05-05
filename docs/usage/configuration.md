# Configuration

This page will describe how to configure an OQTOPUS backend environment.

Configuration files are created under `config/` when you run:

```bash
oqtopus init <env_name> --template backend
```

The generated files are examples for the local backend environment. Review and
update them as needed before starting services.

## Configuration Directory

```text
config/
  .env
  core/
  sse_engine/
  mitigator/
  estimator/
  combiner/
  tranqu/
  gateway/
```

## Environment Variables

`config/.env` contains environment variables used when launching backend
processes.

These values are applied only to the launched process environment. They are not
persisted to your global shell environment.

`config/.env` is a configuration file and should not contain secrets.

## Environment Name Placeholders

The backend template may use `{{ env_name }}` in `config/.env`.

When you run:

```bash
oqtopus init my-demo --template backend
```

OQTOPUS CLI replaces the placeholder with the validated environment name.

For example:

```env
SSE_CONTAINER_IMAGE="{{ env_name }}-sse_runtime:latest"
SSE_CONTAINER_NETWORK="{{ env_name }}_context_app_net"
```

becomes:

```env
SSE_CONTAINER_IMAGE="my-demo-sse_runtime:latest"
SSE_CONTAINER_NETWORK="my-demo_context_app_net"
```

These values are used by Docker, so `env_name` must be Docker-safe:

```text
^[a-z0-9][a-z0-9_.-]*$
```

## Service Configuration

Each service has its own configuration directory under `config/`.

Detailed configuration guidance will be added as the supported options are
documented.
