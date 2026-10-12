# Managing The Manager Component

This page describes component management for `--template manager`
environments. For backend component management, see
[Managing Backend Components](./backend-components.md). For cloud-local
component management, see [Managing Cloud-Local Components](./cloud-local-components.md).

OQTOPUS CLI manages a single component for this template:

- `manager` [https://github.com/oqtopus-team/oqtopus-manager](https://github.com/oqtopus-team/oqtopus-manager)

Because there is only one component, `install`, `uninstall`, `update`, and
`versions` do not take a component name argument, unlike `oqtopus backend` and
`oqtopus cloud-local`.

Release installs are stored under the manager installation root
(`~/.local/share/oqtopus/manager/releases/`), shared across manager
environments on the same machine.

Branch installs clone the repository directly into `$ENV_ROOT/manager/` and
are local to that environment.

## List Available Versions

```bash
oqtopus manager versions
```

Example output:

```text
manager:
* branch:develop (installed)
  v0.2.0 (installed)
  v0.1.0
```

The command reads remote GitHub tags and shows stable semantic version tags in
`vX.Y.Z` format. Pre-release tags are not shown.

You can run this command from any directory. It does not require a manager
environment. When run inside a manager environment, the list also shows local
state:

- `*` marks the version bound in the current environment's `.metadata`.
- `(installed)` marks a release directory already available under
  `install_root`.
- A branch install appears at the top of the list with the format
  `branch:<branch> (installed)`.

## Install The Latest Release

```bash
oqtopus manager install
```

## Install A Specific Version

```bash
oqtopus manager install v0.1.0
```

## Install From A Branch

```bash
oqtopus manager install branch:develop
```

This is intended for development and testing of pre-release features.

Unlike a release install, a branch install:

- Downloads the branch's current commit as a tarball into `$ENV_ROOT/manager`
  instead of the shared installation root.
- Always removes the existing directory and re-downloads on repeated runs, so
  you always get the latest HEAD of the branch.
- Records the branch name in `.metadata`: `manager_version=branch:develop`.

`oqtopus manager start` automatically uses `$ENV_ROOT/manager` when a branch
version is bound, with no additional configuration required.

To remove a branch install:

```bash
oqtopus manager uninstall branch:develop
```

This removes `$ENV_ROOT/manager` and clears the `manager_version` binding from
`.metadata`. Unlike release uninstall, the binding is also removed because
there is no fallback once the directory is deleted.

## Update To The Latest Release

```bash
oqtopus manager update
```

`update` is equivalent to installing the latest stable version and updating
the environment binding.

## Uninstall A Release

```bash
oqtopus manager uninstall v0.1.0
```

This removes the selected local release directory from `install_root`. The
CLI does not check whether the version is used by another manager environment.

## Installation Details

Install and update both download the tagged release source archive and run
`uv sync --frozen --no-dev --project <target_dir>` to build an isolated
virtual environment for that release. `uv` must be installed. `uv` provisions
the Python interpreter version required by the release automatically if it is
not already available.

There is no Docker build step for manager, unlike `engine`'s `sse_runtime`
image in the backend template.

## Configuration Files

`install`, `update`, and `uninstall` do not modify files under `config/` or
`assets/`. See [Manager Configuration](./manager-configuration.md).
