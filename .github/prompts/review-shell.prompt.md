# Review shell script changes

Review the shell script changes in this repository (`bin/oqtopus`, `scripts/install.sh`).

## Safety and correctness

- Every script must start with `set -euo pipefail`
- Variables must be quoted; use `"${var}"` rather than bare `$var`
- Always use `need_command <tool>` before invoking any external command
- Use `die()` for fatal errors, `warn()` for non-fatal warnings, `log()` for informational output — never write to stdout/stderr directly
- Avoid `eval`, `source`-ing untrusted input, or unvalidated external data

## Portability

- Target both Linux (glibc) and macOS (BSD userland)
- Avoid GNU-only flags (e.g., `sed -i ''` vs `sed -i`)
- Use `command -v` instead of `which`
- Prefer `printf` over `echo` for portability

## Structure and readability

- Group related logic into named functions with clear, single responsibilities
- Keep section boundaries marked with `# ---` comment dividers matching the existing style
- Avoid duplicating logic that already exists in a helper function
- New constants (repos, service lists, order arrays) belong at the top of `bin/oqtopus` alongside existing ones

## Behavior consistency

- `start`, `stop`, `restart` must follow the defined service order:
  - Start: `gateway → tranqu → mitigator → estimator → combiner → sse_engine → core`
  - Stop: reverse of start order
- `status` output format must remain consistent with the existing implementation
- Every new subcommand must have a corresponding `usage_*` function and be listed in the relevant `usage_*` help text
- Error messages must be actionable (tell the user what to do, not just what went wrong)

## Documentation

- If a new command, flag, or behavior is added or changed, note whether `docs/usage/command-reference.md` also needs updating
- If install or setup steps change, check `docs/usage/` for affected pages

## Review style

Point out only real issues. Suggest minimal, targeted changes — do not propose full rewrites.
