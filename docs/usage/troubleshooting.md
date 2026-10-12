# Troubleshooting

This page lists common issues and how to recover from them.

## `oqtopus: command not found`

The install directory may not be on your `PATH`.

By default, `oqtopus` is installed to:

```text
~/.local/bin
```

Add that directory to your shell `PATH`, then restart your shell.

## `.metadata not found`

Backend commands must be run from inside a backend environment created by:

```bash
oqtopus init <env_name> --template backend
```

Change into the environment root before running backend commands:

```bash
cd <env_name>
oqtopus backend status
```

## Current Directory Does Not Match `environment_root`

The current directory must match the `environment_root` recorded in `.metadata`.

This can happen if the environment directory was moved after creation. Create a
new environment or move the directory back to its recorded path.

## Downloads Fail Behind A Proxy Or Private CA

`init`, `install`, `update`, and `versions` download from GitHub over HTTPS.

Certificates are verified against the operating system's trust store, so a
private or TLS-inspection CA installed system-wide is trusted. On Linux, the
`SSL_CERT_FILE` and `SSL_CERT_DIR` variables select a different CA bundle or
directory.

Proxies are read from the same variables curl uses for HTTPS, in this order:
`https_proxy`, `HTTPS_PROXY`, `all_proxy`, `ALL_PROXY`. `http_proxy` is not used.
`http://`, `https://`, `socks4://`, `socks4a://`, `socks5://`, and `socks5h://`
proxies are supported. `no_proxy` (or `NO_PROXY`) takes a comma-separated list of
host names; an entry such as `github.com` also matches its subdomains, and `*`
disables the proxy. IP ranges in CIDR notation are not supported.

If the proxy port is omitted, it defaults to `1080`. Credentials embedded in the
proxy URL are percent-decoded: `http://user:p%40ss@proxy.example.com:3128` uses
the password `p@ss`. A literal `+` stays `+`.

There is a known limitation in the HTTP library, ureq, when handling proxy
credentials. Credentials that decode to `/`, `?`, or `#`, spaces, or non-ASCII
characters cannot currently be used. SOCKS5 and SOCKS5h passwords containing
`:` also fail. Settings that alter the decoded username or password are rejected
rather than used with a different proxy address or altered credentials.
HTTP and HTTPS proxy passwords may
contain `:`, but HTTP Basic authentication cannot represent `:` in a username.

```bash
export https_proxy=http://proxy.example.com:3128
oqtopus backend versions
```

## Docker Is Required For Engine Install

Installing `engine` builds the `sse_runtime` Docker image.

If Docker is not installed or not running, commands such as these fail:

```bash
oqtopus backend install engine
oqtopus backend install all
```

Install and start Docker, then run the command again.

If you need to finish installation before Docker is available, use
`--skip-sse-build` and build the image later:

```bash
oqtopus backend install engine --skip-sse-build
oqtopus backend build sse-runtime
```

## `uv` Is Required

Backend and cloud-local components are synchronized and launched with `uv`.

Install `uv` before installing or starting components.

## Branch Name Not Found (Backend)

If the specified branch does not exist in the remote repository, resolving it
fails with an error such as:

```text
Error: failed to resolve branch 'develop' for engine.
```

`branch:<branch>` only accepts a branch name, not a tag. Check that the branch
name is correct, then run the command again with the correct branch name:

```bash
oqtopus backend install engine branch:<correct-branch>
```

## Backend: Component Version Is Not Bound

If a service is started before its component is installed, `start` fails.

Install the required components:

```bash
oqtopus backend install all
```

Then start services again:

```bash
oqtopus backend start all
```

## Service Appears Already Running

If a PID file exists and the recorded process is alive, `start` skips that
service and logs a message such as:

```text
core is already running (PID 12345); skipping.
```

This means `start all` is safe to run at any time: services already running
are skipped and the rest are started normally.

If another `start` command is already launching the same service, `start` fails
before creating a duplicate process.

Check status:

```bash
oqtopus backend status
```

Stop the service if needed:

```bash
oqtopus backend stop <service>
```

## Stop Times Out

`stop` sends `TERM` and waits up to 5 seconds. If the process is still running
after that, `stop` exits with an error.

Inspect the process and stop it manually if required.

## Logs Are Missing

OQTOPUS CLI creates log directories, but it does not create log files.

For backend environments, log files appear only after the backend applications
write them according to the `logging.yaml` files under `config/`.

For cloud-local environments, runtime stdout and stderr are written to
`$ENV_ROOT/logs/<service>/service.log` for each service started in background
mode.

## Docker Compose Is Required For Cloud-Local DB

The `db` service in a cloud-local environment uses Docker Compose:

```bash
oqtopus cloud-local start db
```

If Docker or Docker Compose is not installed or not running, this command
fails. Install and start Docker, then run the command again.

## Cloud-Local DB Container Name Conflict

If `docker compose up` fails with a container name conflict, a container from
a previous run may still exist. Remove it with:

```bash
docker rm -f <container-name>
```

Then run `oqtopus cloud-local start db` again. To avoid conflicts, the
`compose.yaml` in the `cloud` component should not use fixed `container_name`
values.
