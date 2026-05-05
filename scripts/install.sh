#!/bin/sh

set -eu

REPO_OWNER="oqtopus-team"
REPO_NAME="oqtopus-cli"
DEFAULT_BIN_DIR="$HOME/.local/bin"
VERSION=""
BIN_DIR="$DEFAULT_BIN_DIR"

log() {
  printf '%s\n' "$*"
}

progress() {
  printf '%s\n' "$*" >&2
}

warn() {
  printf 'Warning: %s\n' "$*" >&2
}

error() {
  printf 'Error: %s\n' "$*" >&2
}

usage() {
  cat <<'EOF'
Usage: install.sh [--bin-dir <path>] [--version <tag>] [--help]

Options:
  --bin-dir <path>  Install oqtopus into this directory.
           Default: ~/.local/bin
  --version <tag>   Install a specific oqtopus-cli tag, such as vX.Y.Z.
           If omitted, the latest stable vX.Y.Z tag is installed.
  --help            Show this help.
EOF
}

die() {
  error "$*"
  exit 1
}

require_command() {
  if ! command -v "$1" >/dev/null 2>&1; then
    die "'$1' is required but was not found on PATH."
  fi
}

expand_path() {
  case "$1" in
    "~")
      printf '%s\n' "$HOME"
      ;;
    "~/"*)
      printf '%s/%s\n' "$HOME" "${1#~/}"
      ;;
    *)
      printf '%s\n' "$1"
      ;;
  esac
}

cleanup() {
  if [ "${TMP_DIR:-}" ] && [ -d "$TMP_DIR" ]; then
    rm -rf "$TMP_DIR"
  fi
}

parse_args() {
  while [ "$#" -gt 0 ]; do
    case "$1" in
      --bin-dir)
        [ "$#" -ge 2 ] || die "--bin-dir requires a path."
        BIN_DIR=$2
        shift 2
        ;;
      --version)
        [ "$#" -ge 2 ] || die "--version requires a tag."
        VERSION=$2
        shift 2
        ;;
      --help|-h)
        usage
        exit 0
        ;;
      *)
        die "unknown option: $1"
        ;;
    esac
  done
}

latest_version_from_tags() {
  tags_json=$1

  tag_count=$(printf '%s\n' "$tags_json" | sed -n 's/.*"name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' | wc -l | tr -d ' ')
  if [ "$tag_count" = "100" ]; then
    warn "the GitHub tags API returned 100 tags."
    warn "Additional tags may exist, so latest version resolution may be incomplete."
  fi

  latest=$(
    printf '%s\n' "$tags_json" |
      sed -n 's/.*"name"[[:space:]]*:[[:space:]]*"\([^"]*\)".*/\1/p' |
      awk '
        /^v[0-9]+\.[0-9]+\.[0-9]+$/ {
          version = substr($0, 2)
          n = split(version, parts, ".")
          if (n != 3) {
              next
          }
          major = parts[1] + 0
          minor = parts[2] + 0
          patch = parts[3] + 0
          if (!found ||
              major > best_major ||
              (major == best_major && minor > best_minor) ||
              (major == best_major && minor == best_minor && patch > best_patch)) {
              found = 1
              best_major = major
              best_minor = minor
              best_patch = patch
              best = $0
          }
        }
        END {
          if (found) {
              print best
          }
        }
      '
  )

  [ -n "$latest" ] || return 1
  printf '%s\n' "$latest"
}

resolve_latest_version() {
  tags_url="https://api.github.com/repos/$REPO_OWNER/$REPO_NAME/tags?per_page=100"
  tags_file="$TMP_DIR/tags.json"

  progress "Resolving latest OQTOPUS CLI version..."
  if ! curl -fsSL "$tags_url" -o "$tags_file"; then
    die "failed to query GitHub tags API: $tags_url"
  fi

  tags_json=$(cat "$tags_file")
  latest_version_from_tags "$tags_json" || die "could not resolve the latest stable version from GitHub tags."
}

download_archive() {
  archive_url="https://github.com/$REPO_OWNER/$REPO_NAME/archive/refs/tags/$VERSION.tar.gz"
  archive_path="$TMP_DIR/oqtopus-cli-$VERSION.tar.gz"

  progress "Installing OQTOPUS CLI $VERSION..."
  if ! curl -fsSL "$archive_url" -o "$archive_path"; then
    die "failed to download archive for version '$VERSION'."
  fi

  printf '%s\n' "$archive_path"
}

extract_archive() {
  archive_path=$1
  extract_dir="$TMP_DIR/extract"

  mkdir -p "$extract_dir" || die "failed to create temporary extraction directory."

  if ! tar -xzf "$archive_path" -C "$extract_dir"; then
    die "failed to extract archive for version '$VERSION'."
  fi

  source_bin=$(find "$extract_dir" -type f -path "*/bin/oqtopus" | head -n 1)
  [ -n "$source_bin" ] || die "bin/oqtopus was not found in the archive for version '$VERSION'."

  printf '%s\n' "$source_bin"
}

install_binary() {
  source_bin=$1
  BIN_DIR=$(expand_path "$BIN_DIR")

  mkdir -p "$BIN_DIR" || die "failed to create target directory: $BIN_DIR"
  [ -d "$BIN_DIR" ] || die "target path is not a directory: $BIN_DIR"
  [ -w "$BIN_DIR" ] || die "target directory is not writable: $BIN_DIR"

  target_bin="$BIN_DIR/oqtopus"

  if ! cp "$source_bin" "$target_bin"; then
    die "failed to install oqtopus to $target_bin."
  fi

  target_tmp="$target_bin.tmp.$$"
  if sed "s|^CLI_VERSION=.*|CLI_VERSION=\"\${OQTOPUS_CLI_VERSION:-$VERSION}\"|" "$target_bin" > "$target_tmp"; then
    mv "$target_tmp" "$target_bin"
  else
    rm -f "$target_tmp" || true
    warn "could not embed installed version into $target_bin"
  fi

  if ! chmod +x "$target_bin"; then
    die "failed to mark $target_bin as executable."
  fi

  printf '%s\n' "$target_bin"
}

path_contains_bin_dir() {
  case ":$PATH:" in
    *":$BIN_DIR:"*)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

install_one_completion() {
  shell_name=$1
  destination=$2
  installed_bin=$3

  destination=$(expand_path "$destination")
  destination_dir=$(dirname "$destination")

  if ! mkdir -p "$destination_dir"; then
    warn "could not create completion directory for $shell_name: $destination_dir"
    return 1
  fi

  if ! "$installed_bin" completion "$shell_name" > "$destination"; then
    warn "could not generate $shell_name completion at $destination"
    rm -f "$destination" || true
    return 1
  fi

  log "  $shell_name: $destination"
  return 0
}

install_completions() {
  installed_bin=$1

  log "Shell completion:"
  completion_installed=0

  if install_one_completion "bash" "$HOME/.local/share/bash-completion/completions/oqtopus" "$installed_bin"; then
    completion_installed=1
  fi
  if install_one_completion "zsh" "$HOME/.local/share/zsh/site-functions/_oqtopus" "$installed_bin"; then
    completion_installed=1
  fi
  if install_one_completion "fish" "$HOME/.config/fish/completions/oqtopus.fish" "$installed_bin"; then
    completion_installed=1
  fi

  if [ "$completion_installed" -eq 0 ]; then
    warn "no shell completion files were installed."
  fi

  log "The installer did not modify shell startup files."
}

main() {
  parse_args "$@"

  case "$(uname -s 2>/dev/null || printf unknown)" in
    Linux|Darwin)
      ;;
    *)
      die "unsupported platform. OQTOPUS CLI supports Linux and macOS."
      ;;
  esac

  require_command curl
  require_command tar
  require_command mktemp
  require_command chmod
  require_command mkdir
  require_command cp
  require_command sed
  require_command mv
  require_command find
  require_command dirname

  TMP_DIR=$(mktemp -d "${TMPDIR:-/tmp}/oqtopus-install.XXXXXX") || die "failed to create temporary directory."
  trap cleanup EXIT HUP INT TERM

  if [ -z "$VERSION" ]; then
    VERSION=$(resolve_latest_version)
  fi

  archive_path=$(download_archive)
  source_bin=$(extract_archive "$archive_path")
  installed_bin=$(install_binary "$source_bin")

  log "Installed oqtopus $VERSION"
  log "Binary: $installed_bin"

  install_completions "$installed_bin"

  if ! path_contains_bin_dir; then
    warn "$BIN_DIR is not on PATH."
    warn "Add it to PATH before running oqtopus from a new shell."
  fi
}

main "$@"
