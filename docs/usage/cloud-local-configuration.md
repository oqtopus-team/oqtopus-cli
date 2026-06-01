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
processes. These variables are applied only to the launched process environment;
they are not written to your global shell environment.

### Key Variables

| Variable | Default | Description |
|---|---|---|
| `ENV` | `local` | Execution environment |
| `DB_HOST` | `localhost` | MySQL host |
| `DB_NAME` | `main` | MySQL database name |
| `USER_API_PORT` | `8080` | User API port |
| `PROVIDER_API_PORT` | `8888` | Provider API port |
| `ADMIN_API_PORT` | `8889` | Admin API port |
| `USER_SIGNUP_API_PORT` | `8890` | User Signup API port |
| `STORAGE_DRIVER` | `local:minio` | Storage backend |
| `STORAGE_LOCAL_MINIO_ENDPOINT_URL` | `http://localhost:9000` | MinIO endpoint |
| `STORAGE_LOCAL_MINIO_USERNAME` | `minioadmin` | MinIO username |
| `STORAGE_LOCAL_MINIO_PASSWORD` | `minioadmin` | MinIO password |
| `LOG_LEVEL` | `DEBUG` | Log level |

`config/.env` is a configuration file and should not contain production secrets.
