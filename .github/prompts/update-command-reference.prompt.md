---
name: update-command-reference
description: Update CLI command reference documentation from the current implementation
---

# Update command reference

Update the command reference documentation based on the current CLI implementation.

The goal is to keep the public documentation consistent with the implemented CLI behavior.
Do not invent commands, options, or behavior that are not implemented.

## Scope

Check the current CLI implementation and update documentation for:

- Available top-level commands
- Subcommands
- Options and arguments
- Required and optional parameters
- Default behavior
- Exit behavior if relevant
- User-facing error messages if relevant
- Example usage
- Differences between README and detailed documentation

## Documentation rules

Follow these rules:

- Keep the documentation concise and user-oriented.
- Use clear English.
- Prefer practical examples over long explanations.
- Use fenced code blocks with language identifiers, such as `bash`.
- Do not document planned behavior unless it already exists.
- Do not document commands that are not implemented.
- If the implementation is ambiguous, add a short note instead of guessing.
- Keep command names and option names exactly as implemented.

## Files to check

Check these files if they exist:

- `bin/oqtopus`
- `scripts/*.sh`
- `README.md`
- `docs/**/*.md`
- `mkdocs.yml`

## Output format

Return the result in this format:

1. Implementation summary
   - Summarize the commands and options found in the current implementation.

2. Documentation changes
   - List the documentation files that should be updated.

3. Proposed documentation patch
   - Provide a focused patch or replacement text.

4. Gaps or ambiguities
   - List any behavior that is unclear from the implementation.
