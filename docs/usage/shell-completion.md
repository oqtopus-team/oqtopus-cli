# Shell Completion

OQTOPUS CLI can generate shell completion scripts for bash, zsh, and fish.

```bash
oqtopus completion bash
oqtopus completion zsh
oqtopus completion fish
```

Completion includes commands, subcommands, flags, templates, component names,
service names, and device status values.

Version strings are not completed, including for `oqtopus backend versions`.

## Installer Behavior

The installer attempts to place completion files in standard user-local
locations when possible:

```text
bash:
  ~/.local/share/bash-completion/completions/oqtopus

zsh:
  ~/.local/share/zsh/site-functions/_oqtopus

fish:
  ~/.config/fish/completions/oqtopus.fish
```

The installer may create these directories if they do not exist.

Completion installation failures are warnings and do not make the `oqtopus`
binary installation fail.

The installer does not edit shell startup files automatically.

## Manual Setup

You can write completion output to a location supported by your shell.

For bash:

```bash
oqtopus completion bash > ~/.local/share/bash-completion/completions/oqtopus
```

For zsh:

```bash
oqtopus completion zsh > ~/.local/share/zsh/site-functions/_oqtopus
```

For fish:

```bash
oqtopus completion fish > ~/.config/fish/completions/oqtopus.fish
```

Restart your shell after installing completion files.
