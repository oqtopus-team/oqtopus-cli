<!-- markdownlint-disable MD041 -->
![OQTOPUS logo](./asset/oqtopus-logo.png)

# OQTOPUS CLI (Command Line Interface)

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![slack](https://img.shields.io/badge/slack-OQTOPUS-pink.svg?logo=slack&style=plastic")](https://join.slack.com/t/oqtopus/shared_invite/zt-3bpjb7yc3-Vg8IYSMY1m5wV3DR~TMSnw)

## Overview

**OQTOPUS CLI** is a command line interface for setting up and operating local
OQTOPUS environments.

The CLI provides a single `oqtopus` command for the common lifecycle of local
environments: create an environment, install component releases, start, stop,
and restart services, inspect status, and clean up unused installations.
It is designed to make OQTOPUS operation feel closer to familiar developer
tools such as package managers and service managers, while keeping the
underlying configuration files available for users who need to edit them.
The CLI itself can be installed with a single command.

Two environment templates are supported:

- **`cloud-local`** — for OQTOPUS Cloud running locally (`cloud`, `frontend`, `admin`)
- **`backend`** — for the OQTOPUS backend (`engine`, `tranqu`, `gateway`)

With OQTOPUS CLI, users can:

- create a backend or cloud-local environment from the official template;
- install and update components directly from their GitHub releases;
- start, stop, and restart managed services;
- check process status and environment information;
- keep installed component releases in a shared local data directory so that
  multiple environments reuse the same installation without duplication;
- manage per-environment configuration files separately from the shared
  component installations;
- prepare runtime directories such as logs and PID files for local execution.

Currently, the CLI targets Linux and macOS local workflows. Windows is not
supported. A future Rust implementation is planned separately.

## Usage

If you are using OQTOPUS CLI for the first time, start here:

1. [Installation](./usage/installation.md)
2. [Quick Start](./usage/quick-start.md)

**Cloud-local environment:**

- [Cloud-Local Environment](./usage/cloud-local-environment.md)
- [Cloud-Local Configuration](./usage/cloud-local-configuration.md)
- [Managing Cloud-Local Components](./usage/cloud-local-components.md)
- [Starting and Stopping Cloud-Local Services](./usage/cloud-local-lifecycle.md)

**Backend environment:**

- [Backend Environment](./usage/backend-environment.md)
- [Backend Configuration](./usage/backend-configuration.md)
- [Managing Backend Components](./usage/backend-components.md)
- [Starting and Stopping Backend Services](./usage/backend-lifecycle.md)
- [Backend Device Status](./usage/backend-device-status.md)

**Reference and tools:**

- [Command Reference](./usage/command-reference.md)
- [Shell Completion](./usage/shell-completion.md)
- [Troubleshooting](./usage/troubleshooting.md)

## Developer Guidelines

- [Development Flow](./developer_guidelines/development_flow.md)
- [Setup Development Environment](./developer_guidelines/setup.md)
- [How to Contribute](./CONTRIBUTING.md)
- [Code of Conduct](https://github.com/oqtopus-team/.github/blob/main/CODE_OF_CONDUCT.md)
- [Security](https://github.com/oqtopus-team/.github/blob/main/SECURITY.md)

## Citation

Citation information is also available in the [CITATION](https://github.com/oqtopus-team/oqtopus-cli/blob/main/CITATION.cff) file.

## Contact

You can contact us by creating an issue in this repository or by email:

- [oqtopus-team[at]googlegroups.com](mailto:oqtopus-team[at]googlegroups.com)

## License

OQTOPUS CLI is released under the [Apache License 2.0](https://github.com/oqtopus-team/oqtopus-cli/blob/main/LICENSE).
