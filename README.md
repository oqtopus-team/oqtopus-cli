<!-- markdownlint-disable MD041 -->
![OQTOPUS logo](./docs/asset/oqtopus-logo.png)

# OQTOPUS CLI (Command Line Interface)

[![License](https://img.shields.io/badge/License-Apache_2.0-blue.svg)](https://opensource.org/licenses/Apache-2.0)
[![slack](https://img.shields.io/badge/slack-OQTOPUS-pink.svg?logo=slack&style=plastic")](https://join.slack.com/t/oqtopus/shared_invite/zt-3bpjb7yc3-Vg8IYSMY1m5wV3DR~TMSnw)

## Overview

**OQTOPUS CLI** is a command line interface for preparing and operating a local
OQTOPUS backend environment.

It gives OQTOPUS users a familiar command-line workflow for creating backend
environments, installing backend components, and starting or stopping local
services from one `oqtopus` command. Instead of manually arranging release
archives, configuration files, process IDs, and runtime directories, users can
manage the local backend with package-manager-like commands such as:

```bash
oqtopus init demo --template backend
oqtopus backend install engine
oqtopus backend start core
oqtopus backend status
oqtopus backend stop core
```

For v1.0.0, OQTOPUS CLI focuses on Linux and macOS local backend workflows.
Windows support and a future Rust implementation are planned separately.

## Documentation

- [Documentation Home](https://oqtopus-cli.readthedocs.io/)

## Citation

Citation information is also available in the [CITATION](https://github.com/oqtopus-team/oqtopus-cli/blob/main/CITATION.cff) file.

## Contact

You can contact us by creating an issue in this repository or by email:

- [oqtopus-team[at]googlegroups.com](mailto:oqtopus-team[at]googlegroups.com)

## License

OQTOPUS CLI is released under the [Apache License 2.0](https://github.com/oqtopus-team/oqtopus-cli/blob/main/LICENSE).
