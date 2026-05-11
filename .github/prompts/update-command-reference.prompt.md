# Update command reference

Update `docs/usage/command-reference.md` to reflect the current state of `bin/oqtopus`.

## Source of truth

- **Implementation**: `bin/oqtopus` — the `usage_*` functions define the authoritative command signatures
- **Documentation target**: `docs/usage/command-reference.md`

Do not document commands, flags, or behaviors that are not implemented in `bin/oqtopus`.

## What to check

### Commands and subcommands

- Top-level commands (`init`, `backend`, `completion`, `version`, `help`)
- `backend` subcommands (`install`, `versions`, `uninstall`, `update`, `start`, `stop`, `restart`, `status`, `device-status`, `info`)
- All valid values for enumerated arguments (component names, service names, device status values)

### Flags and options

- `--template backend` for `init`
- `--foreground` for `backend start` (single service only)
- Any new flags added to `bin/oqtopus`

### Behavior notes worth documenting

- `install all` installs the latest stable release of each component independently
- `versions` marks the current `.metadata` binding with `*` and locally available releases with `(installed)` when run inside a backend environment
- `--foreground` is only valid for a single service target, not `all`
- `uninstall` removes the release directory without checking cross-environment references
- Service start/stop order (start: `gateway → tranqu → mitigator → estimator → combiner → sse_engine → core`; stop: reverse)

## Style rules

- Use fenced `bash` code blocks for all command examples
- Keep descriptions concise and user-oriented (one sentence per command is enough)
- Group commands under the same headings as the current document
- Do not add a heading for every individual flag — fold flags into their parent command section
- Run `make docs-lint` after editing to verify Markdown formatting
