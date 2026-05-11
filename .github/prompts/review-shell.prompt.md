---
name: review-shell
description: Review Bash script changes for safety, portability, and CLI behavior
---

# Review shell script changes

Review the current shell script changes in this repository.

This repository currently uses Bash scripts for the CLI implementation.
Focus on practical issues that may affect correctness, portability, maintainability,
or user experience.

## Review focus

Check the following points:

- Bash safety
  - Keep `set -euo pipefail` where appropriate.
  - Quote variables unless word splitting is intentional.
  - Handle unset variables and missing arguments safely.
  - Avoid unsafe `eval`, unquoted command substitution, and fragile globbing.

- Error handling
  - Use existing helper functions such as `die`, `warn`, `log`, or similar if available.
  - Return clear error messages for user mistakes.
  - Preserve useful exit codes.
  - Do not hide failures from install/start/stop/status commands.

- Linux/macOS compatibility
  - Avoid GNU-only options unless already used intentionally.
  - Be careful with `sed`, `readlink`, `realpath`, `mktemp`, and path handling.
  - Avoid assumptions about the user's shell environment.

- CLI behavior consistency
  - Keep command names, subcommands, options, and messages consistent.
  - Check install/start/stop/status behavior.
  - Check whether help text should be updated.
  - Do not suggest commands that are not implemented.

- Maintainability
  - Prefer small functions with clear responsibilities.
  - Avoid duplicated logic.
  - Keep changes minimal and easy to review.
  - Preserve the current Bash-based implementation unless the user asks for a rewrite.

- Documentation impact
  - Point out whether README, command reference, or usage docs should be updated.

## Output format

Return the review in this format:

1. Summary
   - Briefly summarize whether the change looks safe.

2. Must fix
   - List correctness, safety, or portability issues that should be fixed before merging.

3. Should consider
   - List maintainability or UX improvements.

4. Documentation updates
   - List documentation files that should be updated, if any.

5. Suggested patch
   - Provide small patches only when they are clearly useful.
   - Do not rewrite the entire script unless explicitly asked.
