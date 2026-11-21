# Cloud-Local Configuration

This page describes configuration for `--template cloud-local` environments.
For backend configuration, see [Configuration](./backend-configuration.md).

Configuration files are created under `config/` when you run:

```bash
oqtopus init <env_name> --template cloud-local
```

Review and update the generated files as needed before starting services.

## Configuration Directory

```text
config/
  .env
```

## Environment Variables

`config/.env` contains environment variables used when launching cloud-local
processes and the database/storage infra command. They are applied only to the
launched process environment, not to your global shell environment.

### Key Variables

| Variable | Default | Description |
|---|---|---|
| `ENV` | `local` | Execution environment |
| `DB_HOST` | `localhost` | MySQL host |
| `DB_NAME` | `main` | MySQL database name |
| `DB_USERNAME` | `admin` | MySQL username |
| `DB_PASSWORD` | `password` | MySQL password |
| `USER_API_PORT` | `8080` | User API port |
| `PROVIDER_API_PORT` | `8888` | Provider API port |
| `ADMIN_API_PORT` | `8889` | Admin API port |
| `USER_SIGNUP_API_PORT` | `8890` | User Signup API port |
| `STORAGE_STACK` | `seaweedfs` | Local object-storage stack (`seaweedfs` or `minio`) |
| `STORAGE_SEAWEEDFS_ENDPOINT_URL` | `http://localhost:8333` | SeaweedFS endpoint |
| `STORAGE_SEAWEEDFS_PUBLIC_ENDPOINT_URL` | `http://localhost:8333` | SeaweedFS endpoint exposed to clients |
| `STORAGE_LOCAL_MINIO_ENDPOINT_URL` | `http://localhost:9000` | MinIO endpoint |
| `STORAGE_LOCAL_MINIO_USERNAME` | `minioadmin` | MinIO username |
| `STORAGE_LOCAL_MINIO_PASSWORD` | `minioadmin` | MinIO password |
| `LOG_LEVEL` | `DEBUG` | Log level |

`oqtopus-cloud-infra` derives the storage driver from `STORAGE_STACK`; do not set
`STORAGE_DRIVER` directly. When changing `STORAGE_STACK`, update the corresponding
storage endpoint and credentials as well.

### API Startup Options

The scopes are `USER_API`, `PROVIDER_API`, `ADMIN_API`, and `USER_SIGNUP_API`.
For each scope, the template sets `<SCOPE>_HOST=0.0.0.0`, `<SCOPE>_RELOAD=true`,
and `<SCOPE>_LOG_LEVEL=debug`, as well as the ports listed above.
`<SCOPE>_POWERTOOLS_SERVICE_NAME` and `<SCOPE>_POWERTOOLS_METRICS_NAMESPACE`
configure Powertools for each API. Other uvicorn options can be passed as
`<SCOPE>_<OPTION>`, for example `USER_API_WORKERS`.

### Existing Environments

`install` and `update` do not modify `config/.env`. When upgrading an existing
cloud-local environment, compare it with the current
[cloud-local template](https://github.com/oqtopus-team/oqtopus-cli/blob/main/templates/cloud-local/config/.env)
and add `STORAGE_STACK`, `DB_USERNAME`, `DB_PASSWORD`, the selected storage
settings, and the API startup settings above. Preserve any customized values.
Without explicit host and reload settings, the cloud API commands default to
`127.0.0.1` and no reload.

The infra command always sets `COMPOSE_PROJECT_NAME` to the environment name,
so its database and storage containers remain isolated from other environments.

`config/.env` is a configuration file and should not contain production secrets.
