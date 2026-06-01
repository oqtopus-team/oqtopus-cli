# Quick Start

This page covers two workflows: **cloud-local** and **backend**. Choose the
one that matches your use case.

## Cloud-Local Quick Start

This guide creates a local cloud-local environment, installs the cloud-local
components, starts all managed services, checks their status, and stops them.

### Create A Cloud-Local Environment

```bash
oqtopus init my-cloud --template cloud-local
cd my-cloud
```

### Install Cloud-Local Components

```bash
oqtopus cloud-local install all
```

This installs the latest stable releases of:

- `cloud` [https://github.com/oqtopus-team/oqtopus-cloud](https://github.com/oqtopus-team/oqtopus-cloud)
- `frontend` [https://github.com/oqtopus-team/oqtopus-frontend](https://github.com/oqtopus-team/oqtopus-frontend)
- `admin` [https://github.com/oqtopus-team/oqtopus-admin](https://github.com/oqtopus-team/oqtopus-admin)

### Start Cloud-Local Services

```bash
oqtopus cloud-local start all
```

The `db` service is started first via Docker Compose. The remaining services
(`worker`, `user_signup`, `admin`, `provider`, `user`) are started as managed
processes.

### Check Cloud-Local Status

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

### Stop Cloud-Local Services

```bash
oqtopus cloud-local stop all
```

Services are stopped in the reverse of the start order. `db` is stopped last
via `docker compose down`.

### Cloud-Local Next Steps

- Learn what `init` creates in [Cloud-Local Environment](./cloud-local-environment.md).
- Install, update, or remove components in [Managing Cloud-Local Components](./cloud-local-components.md).
- Start and stop individual services in [Starting and Stopping Cloud-Local Services](./cloud-local-lifecycle.md).

---

## Backend Quick Start

This guide creates a local backend environment, installs the backend
components, starts all managed services, checks their status, and stops them.

## Create A Backend Environment

```bash
oqtopus init my-backend --template backend
cd my-backend
```

`oqtopus init` creates the local environment directory and prepares
configuration examples, log directories, PID storage, and the `sse_work`
directory.

The environment name is also used to generate Docker-related configuration
values such as Docker image and network names. Use a Docker-safe name with
lowercase letters, digits, `.`, `_`, or `-`.

Backend code is not installed during `init`.

## Install Backend Components

```bash
oqtopus backend install all
```

This installs the latest stable releases of:

- `engine` [https://github.com/oqtopus-team/oqtopus-engine](https://github.com/oqtopus-team/oqtopus-engine)
- `tranqu` [https://github.com/oqtopus-team/tranqu-server](https://github.com/oqtopus-team/tranqu-server)
- `gateway` [https://github.com/oqtopus-team/device-gateway](https://github.com/oqtopus-team/device-gateway)

Each component resolves its own latest version independently.

These are installable component targets. The `engine` component provides
multiple managed services, so the service targets used by `start`, `stop`, and
`restart` include `core`, `sse_engine`, `mitigator`, `estimator`, and
`combiner` in addition to `tranqu` and `gateway`.

To see available versions before installing a specific component version, use:

```bash
oqtopus backend versions engine
```

For development or testing pre-release features, you can install a component
directly from a GitHub branch instead of a release:

```bash
oqtopus backend install engine branch:develop
```

See [Managing Backend Components](./backend-components.md) for details.

## Start Backend Services

```bash
oqtopus backend start all
```

This starts all managed backend services in the required order.

## Check Backend Status

```bash
oqtopus backend status
```

Example output:

```text
core: Running (PID 12345)
sse_engine: Running (PID 12346)
mitigator: Running (PID 12347)
estimator: Running (PID 12348)
combiner: Running (PID 12349)
tranqu: Running (PID 12350)
gateway: Running (PID 12351)
```

## Stop Backend Services

```bash
oqtopus backend stop all
```

Services are stopped in the reverse of the start order.

## Backend Next Steps

- Learn what `init` creates in [Backend Environment](./backend-environment.md).
- Install, update, or remove components in [Managing Backend Components](./backend-components.md).
- Start and stop individual services in [Starting and Stopping Services](./backend-lifecycle.md).
