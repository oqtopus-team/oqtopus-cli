#!/usr/bin/env bash

set -euo pipefail

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
REPO_ROOT="$(cd "$SCRIPT_DIR/.." && pwd)"
TEMPLATE_DIR="$REPO_ROOT/templates/backend/config"

ENGINE_REPO="oqtopus-team/oqtopus-engine"
TRANQU_REPO="oqtopus-team/tranqu-server"
GATEWAY_REPO="oqtopus-team/device-gateway"

SERVICES=(core sse_engine combiner estimator mitigator tranqu gateway)

_ENGINE_VERSION=""
_TRANQU_VERSION=""
_GATEWAY_VERSION=""
_TMPFILE=""

cleanup() { [[ -z "$_TMPFILE" ]] || rm -f "$_TMPFILE"; }
trap cleanup EXIT

# -----------------------------------------------------------------------------
# Helpers
# -----------------------------------------------------------------------------

die()          { printf 'Error: %s\n' "$*" >&2; exit 1; }
need_command() { command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not found on PATH."; }

is_in_list() {
  local needle=$1; shift
  local item
  for item in "$@"; do [[ "$item" == "$needle" ]] && return 0; done
  return 1
}

is_branch_version() { [[ "$1" == branch:* ]]; }

version_to_ref() {
  if is_branch_version "$1"; then printf '%s\n' "${1#branch:}"
  else printf '%s\n' "$1"
  fi
}

resolve_latest_version() {
  local repo=$1 tags_json latest
  tags_json=$(curl -fsSL "https://api.github.com/repos/${repo}/tags?per_page=100") \
    || die "failed to query GitHub tags API for $repo."
  latest=$(jq -r \
    '[.[].name as $t | $t
      | select(test("^v[0-9]+\\.[0-9]+\\.[0-9]+$"))
      | capture("^v(?<a>[0-9]+)\\.(?<b>[0-9]+)\\.(?<c>[0-9]+)$")
      | {t: $t, a: (.a|tonumber), b: (.b|tonumber), c: (.c|tonumber)}]
    | sort_by([.a, .b, .c]) | last | .t // empty' <<< "$tags_json")
  [[ -n "$latest" ]] || die "no stable release found for $repo."
  printf '%s\n' "$latest"
}

service_repo() {
  case "$1" in
    core|sse_engine|combiner|estimator|mitigator) printf '%s\n' "$ENGINE_REPO" ;;
    tranqu)  printf '%s\n' "$TRANQU_REPO" ;;
    gateway) printf '%s\n' "$GATEWAY_REPO" ;;
    *)       die "unknown service: $1" ;;
  esac
}

cached_latest_version() {
  local repo=$1
  case "$repo" in
    "$ENGINE_REPO")
      [[ -n "$_ENGINE_VERSION" ]] || _ENGINE_VERSION=$(resolve_latest_version "$repo")
      printf '%s\n' "$_ENGINE_VERSION" ;;
    "$TRANQU_REPO")
      [[ -n "$_TRANQU_VERSION" ]] || _TRANQU_VERSION=$(resolve_latest_version "$repo")
      printf '%s\n' "$_TRANQU_VERSION" ;;
    "$GATEWAY_REPO")
      [[ -n "$_GATEWAY_VERSION" ]] || _GATEWAY_VERSION=$(resolve_latest_version "$repo")
      printf '%s\n' "$_GATEWAY_VERSION" ;;
  esac
}

# -----------------------------------------------------------------------------
# File mappings: output "<local_rel_path> <upstream_path>" per line
# -----------------------------------------------------------------------------

service_file_mappings() {
  case "$1" in
    core)
      printf '%s\n' "core/config.yaml core/config/config.yaml"
      printf '%s\n' "core/logging.yaml core/config/logging.yaml"
      ;;
    sse_engine)
      printf '%s\n' "sse_engine/config.yaml core/config/sse_engine_config.yaml"
      printf '%s\n' "sse_engine/logging.yaml core/config/sse_engine_logging.yaml"
      ;;
    combiner)
      printf '%s\n' "combiner/config.yaml combiner/config/config.yaml"
      printf '%s\n' "combiner/logging.yaml combiner/config/logging.yaml"
      ;;
    estimator)
      printf '%s\n' "estimator/config.yaml estimator/config/config.yaml"
      printf '%s\n' "estimator/logging.yaml estimator/config/logging.yaml"
      ;;
    mitigator)
      printf '%s\n' "mitigator/config.yaml mitigator/config/config.yaml"
      printf '%s\n' "mitigator/logging.yaml mitigator/config/logging.yaml"
      ;;
    tranqu)
      printf '%s\n' "tranqu/config.yaml config/config.yaml"
      printf '%s\n' "tranqu/logging.yaml config/logging.yaml"
      ;;
    gateway)
      printf '%s\n' "gateway/config.yaml config/config.yaml.qulacs"
      printf '%s\n' "gateway/logging.yaml config/logging.yaml"
      printf '%s\n' "gateway/device_topology_sim.json config/example/device_topology_sim.json"
      ;;
  esac
}

# -----------------------------------------------------------------------------
# Comparison
# -----------------------------------------------------------------------------

compare_service() {
  local service=$1 explicit_version=$2 file_filter=$3
  local repo version ref repo_name
  local any_problem=false

  repo=$(service_repo "$service")
  if [[ -n "$explicit_version" ]]; then
    version="$explicit_version"
  else
    version=$(cached_latest_version "$repo")
  fi
  ref=$(version_to_ref "$version")
  repo_name=$(basename "$repo")

  _TMPFILE=$(mktemp)
  local matched=false

  while IFS=' ' read -r local_rel upstream_rel; do
    local filename
    filename=$(basename "$local_rel")
    [[ -z "$file_filter" || "$filename" == "$file_filter" ]] || continue

    matched=true
    local local_file="$TEMPLATE_DIR/$local_rel"
    local raw_url="https://raw.githubusercontent.com/$repo/$ref/$upstream_rel"
    local diff_output="" diff_exit=0 curl_exit=0

    printf 'comparing: %s  (%s %s)\n' "$local_rel" "$repo_name" "$version"

    if [[ ! -f "$local_file" ]]; then
      printf 'NOT FOUND (local): %s\n\n' "$local_file"
      continue
    fi

    curl -fsSL "$raw_url" -o "$_TMPFILE" 2>/dev/null || curl_exit=$?
    if [[ $curl_exit -ne 0 ]]; then
      printf 'NOT FOUND (upstream): %s\n\n' "$raw_url"
      continue
    fi

    diff_output=$(diff -u \
      --label "templates/backend/config/$local_rel" \
      --label "${repo_name} ${version} (${upstream_rel})" \
      "$local_file" "$_TMPFILE") || diff_exit=$?

    if [[ $diff_exit -eq 0 ]]; then
      printf 'identical\n\n'
    else
      printf '%s\n\n' "$diff_output"
      any_problem=true
    fi

  done < <(service_file_mappings "$service")

  if [[ "$matched" == "false" && -n "$file_filter" ]]; then
    printf 'no files matched --file %s for service %s\n\n' "$file_filter" "$service"
  fi

  rm -f "$_TMPFILE"; _TMPFILE=""
  [[ "$any_problem" == "false" ]]
}

# -----------------------------------------------------------------------------
# Usage
# -----------------------------------------------------------------------------

usage() {
  cat <<'EOF'
Usage:
  diff-backend-configs.sh [<service> [<version>]] [--file <filename>]
  diff-backend-configs.sh completion <bash|zsh|fish>

Compare files in templates/backend/config/ against the corresponding files
in the upstream repositories.

Arguments:
  service    core | sse_engine | combiner | estimator | mitigator | tranqu | gateway
             (default: all services)
  version    v1.2.3  or  branch:<branch>
             (default: latest release tag)
             For engine services (core, sse_engine, combiner, estimator, mitigator),
             this is the oqtopus-engine release version.

Options:
  --file <filename>   Compare only files matching this exact name
  -h, --help          Show this help

Exit status:
  0  All compared files are identical
  1  One or more files differ

Examples:
  diff-backend-configs.sh
  diff-backend-configs.sh core
  diff-backend-configs.sh core v2.0.0
  diff-backend-configs.sh tranqu branch:main
  diff-backend-configs.sh --file config.yaml
  diff-backend-configs.sh core --file logging.yaml
EOF
}

# -----------------------------------------------------------------------------
# Shell completions
# -----------------------------------------------------------------------------

completion_bash() {
  cat <<'BASH'
_diff_backend_configs() {
  local cur prev
  COMPREPLY=()
  cur="${COMP_WORDS[COMP_CWORD]}"
  prev="${COMP_WORDS[COMP_CWORD-1]}"

  if [[ "$prev" == "--file" ]]; then
    COMPREPLY=( $(compgen -W "config.yaml logging.yaml device_topology_sim.json" -- "$cur") )
    return 0
  fi

  if [[ "${COMP_WORDS[1]:-}" == "completion" ]]; then
    COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") )
    return 0
  fi

  local services="core sse_engine combiner estimator mitigator tranqu gateway"
  local has_service=false word
  for word in "${COMP_WORDS[@]:1:$((${#COMP_WORDS[@]}-2))}"; do
    case "$word" in
      core|sse_engine|combiner|estimator|mitigator|tranqu|gateway) has_service=true ;;
    esac
  done

  if [[ "$has_service" == "false" ]]; then
    COMPREPLY=( $(compgen -W "$services completion --file -h --help" -- "$cur") )
  else
    COMPREPLY=( $(compgen -W "--file" -- "$cur") )
  fi
}
complete -F _diff_backend_configs diff-backend-configs diff-backend-configs.sh scripts/diff-backend-configs.sh
BASH
}

completion_zsh() {
  cat <<'ZSH'
#compdef diff-backend-configs

_diff_backend_configs() {
  local -a services=(core sse_engine combiner estimator mitigator tranqu gateway)
  local has_service=false word

  case "${words[-2]}" in
    --file)     compadd -- config.yaml logging.yaml device_topology_sim.json; return ;;
    completion) compadd -- bash zsh fish; return ;;
  esac

  for word in "${words[@]:1:$((${#words[@]}-2))}"; do
    case "$word" in
      core|sse_engine|combiner|estimator|mitigator|tranqu|gateway) has_service=true ;;
    esac
  done

  if [[ "$has_service" == "false" ]]; then
    compadd -- "${services[@]}" completion --file -h --help
  else
    compadd -- --file
  fi
}

compdef _diff_backend_configs diff-backend-configs diff-backend-configs.sh scripts/diff-backend-configs.sh
ZSH
}

completion_fish() {
  cat <<'FISH'
set -l cmds diff-backend-configs diff-backend-configs.sh scripts/diff-backend-configs.sh
set -l services core sse_engine combiner estimator mitigator tranqu gateway
set -l files config.yaml logging.yaml device_topology_sim.json

for cmd in $cmds
  complete -c $cmd -f \
    -n "not __fish_seen_subcommand_from $services completion" \
    -a "$services"
  complete -c $cmd -f \
    -n "not __fish_seen_subcommand_from $services completion" \
    -a "completion --file -h --help"
  complete -c $cmd -f \
    -n "__fish_seen_subcommand_from completion" \
    -a "bash zsh fish"
  complete -c $cmd \
    -n "contains -- --file (commandline -opc)" \
    -a "$files"
end
FISH
}

cmd_completion() {
  [[ $# -eq 1 ]] || { printf 'Usage: diff-backend-configs completion <bash|zsh|fish>\n' >&2; exit 1; }
  case "$1" in
    bash) completion_bash ;;
    zsh)  completion_zsh ;;
    fish) completion_fish ;;
    *)    die "unsupported shell: $1" ;;
  esac
}

# -----------------------------------------------------------------------------
# Main
# -----------------------------------------------------------------------------

main() {
  case "${1:-}" in
    completion) shift; cmd_completion "$@"; return ;;
    -h|--help)  usage; return ;;
  esac

  need_command curl
  need_command diff
  need_command jq

  local service="" version="" file_filter=""

  while [[ $# -gt 0 ]]; do
    case "$1" in
      --file)
        [[ $# -ge 2 ]] || die "--file requires an argument."
        file_filter="$2"
        shift 2 ;;
      -h|--help)
        usage; return ;;
      -*)
        die "unknown option: $1" ;;
      *)
        if [[ -z "$service" ]]; then
          is_in_list "$1" "${SERVICES[@]}" || die "unknown service '$1'. Valid: ${SERVICES[*]}"
          service="$1"
        elif [[ -z "$version" ]]; then
          version="$1"
        else
          die "unexpected argument: $1"
        fi
        shift ;;
    esac
  done

  [[ -z "$version" || -n "$service" ]] || die "a service must be specified when a version is given."

  local services_to_run=()
  if [[ -n "$service" ]]; then
    services_to_run=("$service")
  else
    services_to_run=("${SERVICES[@]}")
  fi

  local any_problem=false
  for svc in "${services_to_run[@]}"; do
    if ! compare_service "$svc" "$version" "$file_filter"; then
      any_problem=true
    fi
  done

  [[ "$any_problem" == "false" ]]
}

main "$@"
