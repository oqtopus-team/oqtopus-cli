# Quick Start

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

## Start Services

```bash
oqtopus backend start all
```

This starts all managed backend services in the required order.

## Check Status

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

## Stop Services

```bash
oqtopus backend stop all
```

Services are stopped in the reverse of the start order.

## Next Steps

- Learn what `init` creates in [Backend Environment](./backend-environment.md).
- Install, update, or remove components in [Managing Backend Components](./backend-components.md).
- Start and stop individual services in [Starting and Stopping Services](./lifecycle.md).
