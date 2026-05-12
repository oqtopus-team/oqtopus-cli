# Managing Backend Components

OQTOPUS CLI manages three backend components:

- `engine`
- `tranqu`
- `gateway`

These names are installable component targets. Each target corresponds to a
separate source repository:

- `engine` [https://github.com/oqtopus-team/oqtopus-engine](https://github.com/oqtopus-team/oqtopus-engine)
- `tranqu` [https://github.com/oqtopus-team/tranqu-server](https://github.com/oqtopus-team/tranqu-server)
- `gateway` [https://github.com/oqtopus-team/device-gateway](https://github.com/oqtopus-team/device-gateway)

`install`, `update`, and `uninstall` operate on these component targets.

Installed releases are stored under the backend installation root recorded in
`.metadata`. Each component version has its own isolated directory and virtual
environment.

The installable components are not the same as the process targets used by
`start` and `stop`. In particular, `engine` is installed as one component, but
it provides multiple managed services such as `core`, `sse_engine`,
`mitigator`, `estimator`, and `combiner`.

The `engine` repository is a monorepo. During `engine` installation, OQTOPUS
CLI synchronizes the uv environments for the `core`, `combiner`, `estimator`,
and `mitigator` subprojects. `sse_engine` uses the installed `core` project.

## List Available Versions

```bash
oqtopus backend versions engine
```

This lists stable versions available for a component:

```text
engine:
* v2.0.3 (installed)
  v2.0.2 (installed)
  v2.0.1
```

The command reads remote GitHub tags and shows stable semantic version tags in
`vX.Y.Z` format. Pre-release tags are not shown.

You can run this command from any directory. It does not require a backend
environment.

When you run it from a backend environment, the list also shows local state:

- `*` marks the version selected by the current environment's `.metadata`.
- `(installed)` marks a release directory already available under
  `install_root`.

This helps you choose an `uninstall` target without checking the installation
directory manually. If a local version is not present in remote stable tags, it
is still shown and marked as `not in remote tags`.

## Install All Components

```bash
oqtopus backend install all
```

This installs `engine`, `tranqu`, and `gateway`.

Each component resolves its own latest stable version independently from its own
repository. Because the components are versioned independently,
`install all` does not accept a version argument.

The install order is:

1. `engine`
2. `tranqu`
3. `gateway`

If one component fails to install, the command stops. Components already
installed successfully remain bound in `.metadata`; rollback is not performed
automatically.

## Install One Component

Install the latest stable version:

```bash
oqtopus backend install engine
```

Install a specific version:

```bash
oqtopus backend install engine v1.2.3
```

The same form applies to `tranqu` and `gateway`.

## Engine And `sse_runtime`

Installing `engine` also builds the `sse_runtime` Docker image from the
installed engine release. This Docker build can take several minutes,
especially the first time it runs or after Docker cache cleanup.

Because `engine` is a monorepo, installing it also prepares the uv environments
for the engine service subprojects:

- `core`
- `combiner`
- `estimator`
- `mitigator`

Docker must be available when running:

```bash
oqtopus backend install engine
```

or:

```bash
oqtopus backend install all
```

To install backend components without building the Docker image, use:

```bash
oqtopus backend install engine --skip-sse-build
```

or:

```bash
oqtopus backend install all --skip-sse-build
```

The flag skips only the `sse_runtime` Docker build. The engine release is still
installed, its uv environments are still prepared, and `.metadata` is still
updated.

To build the image later for the engine version currently bound in `.metadata`,
run:

```bash
oqtopus backend build sse-runtime
```

The Docker build uses `SSE_CONTAINER_IMAGE` from `config/.env`. The build
arguments `UID` and `GID` are set automatically from the current user with
`id -u` and `id -g`; they do not need to be written in `config/.env`.

If Docker is unavailable, `SSE_CONTAINER_IMAGE` is missing, or the Docker build
fails, the engine install fails and the `engine_version` binding is not updated.

## Update One Component

```bash
oqtopus backend update engine
```

`update` is equivalent to installing the latest stable version of the specified
component and updating the environment binding.

## Uninstall A Component Version

```bash
oqtopus backend uninstall engine v1.2.3
```

This removes the selected local release directory from `install_root`.

The CLI does not check whether the version is used by the current environment
or another backend environment. If an environment still references a removed
version, install that version again or update the environment to another
version before starting services.

## Configuration Files

`install`, `update`, and `uninstall` do not modify files under `config/`.

If a component version requires configuration changes, update the configuration
files manually.
