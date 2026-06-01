# Managing Cloud-Local Components

This page describes component management for `--template cloud-local`
environments. For backend component management, see
[Managing Backend Components](./backend-components.md).

OQTOPUS CLI manages three cloud-local components:

- `cloud`
- `frontend`
- `admin`

Each component corresponds to a separate source repository:

- `cloud` [https://github.com/oqtopus-team/oqtopus-cloud](https://github.com/oqtopus-team/oqtopus-cloud)
- `frontend` [https://github.com/oqtopus-team/oqtopus-frontend](https://github.com/oqtopus-team/oqtopus-frontend)
- `admin` [https://github.com/oqtopus-team/oqtopus-admin](https://github.com/oqtopus-team/oqtopus-admin)

The `cloud` component provides all managed services: `db`, `worker`,
`user_signup`, `admin`, `provider`, and `user`. The `frontend` and `admin`
components are installable, but their service start and stop commands are
not yet implemented.

Release installs are stored under the cloud-local installation root
(`~/.local/share/oqtopus/cloud-local/releases/`), shared across environments
on the same machine.

Branch installs clone the repository directly into `$ENV_ROOT/<component>/`
and are local to that environment.

## List Available Versions

```bash
oqtopus cloud-local versions cloud
```

Example output:

```text
cloud:
* branch:develop (installed)
  v1.6.0 (installed)
  v1.5.0
```

Branch installs appear at the top of the list with `(installed)`.

## Install All Components

```bash
oqtopus cloud-local install all
```

This installs the latest stable `cloud`, `frontend`, and `admin` releases
independently. Each component resolves its own latest version from its
repository.

## Install One Component

Install the latest stable version:

```bash
oqtopus cloud-local install cloud
```

Install a specific version:

```bash
oqtopus cloud-local install cloud v1.6.0
```

## Install From A Branch

To install a component directly from a GitHub branch:

```bash
oqtopus cloud-local install cloud branch:develop
```

This is intended for development and testing of pre-release features.

Unlike a release install, a branch install:

- Clones the repository with `git clone --depth 1` into `$ENV_ROOT/<component>/`
  instead of the shared installation root.
- Always removes the existing directory and re-clones on repeated runs.
- Records the branch name in `.metadata`: `cloud_local_cloud_version=branch:develop`

`git` must be installed to use this feature.

To remove a branch install:

```bash
oqtopus cloud-local uninstall cloud branch:develop
```

This removes `$ENV_ROOT/cloud/` and clears the binding from `.metadata`.

## Update One Component

```bash
oqtopus cloud-local update cloud
```

Installs the latest stable version and updates the environment binding.

## Uninstall A Component Version

```bash
oqtopus cloud-local uninstall cloud v1.6.0
```

Removes the selected local release directory from the installation root.

## Configuration Files

`install`, `update`, and `uninstall` do not modify files under `config/`.
