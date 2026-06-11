
# diff-backend-configs.sh

`scripts/diff-backend-configs.sh` is a developer tool that compares the config files in
`templates/backend/config/` against the corresponding files in the upstream repositories
([oqtopus-engine](https://github.com/oqtopus-team/oqtopus-engine),
[tranqu-server](https://github.com/oqtopus-team/tranqu-server),
[device-gateway](https://github.com/oqtopus-team/device-gateway)).

Use this tool to verify that the template config files are in sync with upstream after
updating a component version.

## Prerequisites

| Tool   | Description                     |
| ------ | ------------------------------- |
| `curl` | Fetches upstream files via HTTPS |
| `diff` | Compares file contents          |
| `jq`   | Parses GitHub API responses     |

## Usage

```shell
scripts/diff-backend-configs.sh [<service> [<version>]] [--file <filename>]
scripts/diff-backend-configs.sh completion <bash|zsh|fish>
```

### Arguments

| Argument  | Description |
| --------- | ----------- |
| `service` | `core` \| `sse_engine` \| `combiner` \| `estimator` \| `mitigator` \| `tranqu` \| `gateway` (default: all services) |
| `version` | `v1.2.3` or `branch:<branch>` (default: latest release tag) |

For engine services (`core`, `sse_engine`, `combiner`, `estimator`, `mitigator`),
`version` refers to the `oqtopus-engine` release tag.

### Options

| Option               | Description                                      |
| -------------------- | ------------------------------------------------ |
| `--file <filename>`  | Compare only files matching this exact name      |
| `-h`, `--help`       | Show help                                        |

### Exit Status

| Code | Meaning                            |
| ---- | ---------------------------------- |
| `0`  | All compared files are identical   |
| `1`  | One or more files differ           |

## Examples

```shell
# Compare all services against their latest release tags
scripts/diff-backend-configs.sh

# Compare a single service against its latest release tag
scripts/diff-backend-configs.sh core

# Compare a single service against a specific version
scripts/diff-backend-configs.sh core v2.0.0

# Compare against a branch
scripts/diff-backend-configs.sh tranqu branch:main

# Compare only config.yaml across all services
scripts/diff-backend-configs.sh --file config.yaml

# Compare only logging.yaml for a specific service
scripts/diff-backend-configs.sh core --file logging.yaml
```

You can also run the default check (all services, latest) via Make:

```shell
make diff-backend-configs
```

## Output

Each file is reported as `identical`, `NOT FOUND (local)`, `NOT FOUND (upstream)`,
or a unified diff when differences are found.

```text
comparing: core/config.yaml  (oqtopus-engine v2.1.0)
identical

comparing: core/logging.yaml  (oqtopus-engine v2.1.0)
--- templates/backend/config/core/logging.yaml
+++ oqtopus-engine v2.1.0 (core/config/logging.yaml)
@@ -1,3 +1,3 @@
 ...
```

## Shell Completion

To enable shell completion, source the output of the `completion` subcommand in your shell
configuration file.

=== "bash"

    ```shell
    # Add to ~/.bashrc
    source <(scripts/diff-backend-configs.sh completion bash)
    ```

=== "zsh"

    ```shell
    # Add to ~/.zshrc
    source <(scripts/diff-backend-configs.sh completion zsh)
    ```

=== "fish"

    ```shell
    # Add to ~/.config/fish/config.fish
    scripts/diff-backend-configs.sh completion fish | source
    ```
