# Device Status

The gateway device status can be shown or changed with:

```bash
oqtopus backend device-status show
oqtopus backend device-status active
oqtopus backend device-status inactive
oqtopus backend device-status maintenance
```

These commands operate on:

```text
<env_root>/config/gateway/device_status
```

## Show Current Status

```bash
oqtopus backend device-status show
```

This prints the current value.

## Change Status

Set the device status to active:

```bash
oqtopus backend device-status active
```

Set the device status to inactive:

```bash
oqtopus backend device-status inactive
```

Set the device status to maintenance:

```bash
oqtopus backend device-status maintenance
```

Valid values are only:

- `active`
- `inactive`
- `maintenance`

The command updates the local configuration file directly. It does not require
`gateway` to be running.
