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
      # Quoted so the pattern stays a literal "~/" instead of tilde-expanding to $HOME.
      printf '%s/%s\n' "$HOME" "${1#"~/"}"
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
  tags=$1

  latest=$(
    printf '%s\n' "$tags" |
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

remote_refs_url() {
  printf 'https://github.com/%s/%s.git/info/refs?service=git-upload-pack\n' "$REPO_OWNER" "$REPO_NAME"
}

# Outputs "<sha> refs/..." lines for every ref the remote advertises. Must
# not call die: this is invoked from inside command substitution, where
# exit would only terminate the subshell, so callers check the return
# code themselves.
fetch_remote_refs() {
  refs_file="$TMP_DIR/refs.dat"
  if ! curl -fsSL "$(remote_refs_url)" -o "$refs_file"; then
    return 1
  fi
  # The response is pkt-line framed and the first ref line embeds a NUL
  # before the capability list; reading it via command substitution would
  # make the shell print a "null byte" warning to stderr, so it is read
  # from a file and NULs are stripped before grep sees it.
  LC_ALL=C tr -d '\0' < "$refs_file" | LC_ALL=C grep -ao '[0-9a-f]\{40\} refs/[^ [:cntrl:]]*'
}

# Outputs one tag name per line (annotated tags' peeled "^{}" entries removed).
fetch_remote_tags() {
  tags=$(fetch_remote_refs \
         | sed -n 's|^[0-9a-f]\{40\} refs/tags/||p' \
         | grep -v '{}$' \
         | sort -u) || return 1
  [ -n "$tags" ] || return 1
  printf '%s\n' "$tags"
}

resolve_latest_version() {
  tags_url=$(remote_refs_url)

  progress "Resolving latest OQTOPUS CLI version..."
  tags=$(fetch_remote_tags) || die "failed to query GitHub tags API: $tags_url"

  latest_version_from_tags "$tags" || die "could not resolve the latest stable version from GitHub tags."
}

# v1 releases ship the CLI as a shell script inside the source archive; every
# later release publishes prebuilt binaries instead.
is_legacy_version() {
  case "$1" in
    v1.*)
      return 0
      ;;
    *)
      return 1
      ;;
  esac
}

prepare_install_target() {
  mkdir -p "$BIN_DIR" || die "failed to create target directory: $BIN_DIR"
  [ -d "$BIN_DIR" ] || die "target path is not a directory: $BIN_DIR"
  [ -w "$BIN_DIR" ] || die "target directory is not writable: $BIN_DIR"

  target_bin="$BIN_DIR/oqtopus"
  [ ! -d "$target_bin" ] || die "target executable path is a directory: $target_bin"
  target_tmp="$target_bin.tmp.$$"
}

# Renaming the staged file over the target replaces it even while an older
# oqtopus is running, where writing into the running executable would fail on
# Linux (or corrupt a running v1 script).
commit_install_target() {
  if ! chmod +x "$target_tmp" || ! mv -f "$target_tmp" "$target_bin"; then
    rm -f "$target_tmp" || true
    die "failed to install oqtopus to $target_bin."
  fi
  printf '%s\n' "$target_bin"
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

install_legacy_script() {
  source_bin=$1
  prepare_install_target

  if ! sed "s|^CLI_VERSION=.*|CLI_VERSION=\"\${OQTOPUS_CLI_VERSION:-$VERSION}\"|" "$source_bin" > "$target_tmp"; then
    warn "could not embed installed version into $target_bin"
    if ! cp "$source_bin" "$target_tmp"; then
      rm -f "$target_tmp" || true
      die "failed to install oqtopus to $target_bin."
    fi
  fi

  commit_install_target
}

install_legacy() {
  archive_path=$(download_archive)
  source_bin=$(extract_archive "$archive_path")
  install_legacy_script "$source_bin"
}

# Prints the Rust target triple of the release archive that runs on this machine.
release_target() {
  os=$(uname -s)
  arch=$(uname -m)

  case "$os" in
    Linux)
      # The Linux binaries are statically linked against musl, so they run on any libc.
      case "$arch" in
        x86_64|amd64)
          printf '%s\n' "x86_64-unknown-linux-musl"
          ;;
        aarch64|arm64)
          printf '%s\n' "aarch64-unknown-linux-musl"
          ;;
        *)
          return 1
          ;;
      esac
      ;;
    Darwin)
      # A shell running under Rosetta reports x86_64 on Apple silicon, where the
      # arm64 binary still runs natively.
      if [ "$arch" = "x86_64" ] && [ "$(sysctl -n sysctl.proc_translated 2>/dev/null || true)" = "1" ]; then
        arch=arm64
      fi
      case "$arch" in
        arm64|aarch64)
          printf '%s\n' "aarch64-apple-darwin"
          ;;
        *)
          return 1
          ;;
      esac
      ;;
    *)
      return 1
      ;;
  esac
}

sha256_of() {
  if command -v sha256sum >/dev/null 2>&1; then
    digest=$(sha256sum "$1") || return 1
  elif command -v shasum >/dev/null 2>&1; then
    digest=$(shasum -a 256 "$1") || return 1
  else
    return 1
  fi
  printf '%s\n' "${digest%% *}"
}

release_asset_url() {
  printf 'https://github.com/%s/%s/releases/download/%s/%s\n' "$REPO_OWNER" "$REPO_NAME" "$VERSION" "$1"
}

download_release_asset() {
  asset=$1
  destination=$2

  if ! curl -fsSL "$(release_asset_url "$asset")" -o "$destination"; then
    die "failed to download $asset for version '$VERSION'. Check that the release exists and provides this platform."
  fi
}

verify_release_archive() {
  archive_path=$1
  archive_name=$2
  sums_path=$3

  # SHA256SUMS lists "<digest>  <file>", or "<digest> *<file>" in binary mode.
  expected=$(awk -v name="$archive_name" '$2 == name || $2 == "*" name { print $1; exit }' "$sums_path")
  [ -n "$expected" ] || die "SHA256SUMS for version '$VERSION' has no entry for $archive_name."

  actual=$(sha256_of "$archive_path") || die "failed to compute the SHA-256 digest of $archive_name."
  [ "$actual" = "$expected" ] || die "checksum mismatch for $archive_name."
}

install_release_binary() {
  target=$(release_target) || die "no prebuilt binary is published for $(uname -s) $(uname -m)."
  package="oqtopus-$VERSION-$target"
  archive_name="$package.tar.gz"
  archive_path="$TMP_DIR/$archive_name"
  sums_path="$TMP_DIR/SHA256SUMS"
  extract_dir="$TMP_DIR/extract"

  progress "Installing OQTOPUS CLI $VERSION ($target)..."
  download_release_asset "$archive_name" "$archive_path"
  download_release_asset "SHA256SUMS" "$sums_path"
  verify_release_archive "$archive_path" "$archive_name" "$sums_path"

  mkdir -p "$extract_dir" || die "failed to create temporary extraction directory."
  if ! tar -xzf "$archive_path" -C "$extract_dir"; then
    die "failed to extract $archive_name."
  fi
  source_bin="$extract_dir/$package/oqtopus"
  [ -f "$source_bin" ] || die "$package/oqtopus was not found in $archive_name."

  prepare_install_target
  if ! cp "$source_bin" "$target_tmp"; then
    rm -f "$target_tmp" || true
    die "failed to install oqtopus to $target_bin."
  fi

  commit_install_target
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
  BIN_DIR=$(expand_path "$BIN_DIR")

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

  if is_legacy_version "$VERSION"; then
    installed_bin=$(install_legacy)
  else
    if ! command -v sha256sum >/dev/null 2>&1 && ! command -v shasum >/dev/null 2>&1; then
      die "'sha256sum' or 'shasum' is required to verify the download."
    fi
    installed_bin=$(install_release_binary)
  fi

  log "Installed oqtopus $VERSION"
  log "Binary: $installed_bin"

  install_completions "$installed_bin"

  if ! path_contains_bin_dir; then
    warn "$BIN_DIR is not on PATH."
    warn "Add it to PATH before running oqtopus from a new shell."
  fi
}

main "$@"
