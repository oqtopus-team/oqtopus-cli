//! Shell completion script generation.
//!
//! Candidate lists are maintained alongside the route table. Generated scripts are pinned by
//! compatibility snapshots until completion and parsing share a command model.

use crate::args::is_help;

pub(crate) enum CompletionOutput {
    Usage,
    Script(&'static str),
}

pub(crate) struct CompletionOutcome {
    pub(crate) output: CompletionOutput,
    exit_code: i32,
}

impl CompletionOutcome {
    pub(crate) fn exit_code(&self) -> i32 {
        self.exit_code
    }
}

pub(crate) fn completion(args: &[String]) -> Result<CompletionOutcome, String> {
    if is_help(args) {
        return Ok(CompletionOutcome {
            output: CompletionOutput::Usage,
            exit_code: 0,
        });
    }
    if args.len() != 1 {
        return Ok(CompletionOutcome {
            output: CompletionOutput::Usage,
            exit_code: 1,
        });
    }

    let script = match args[0].as_str() {
        "bash" => BASH_COMPLETION,
        "zsh" => ZSH_COMPLETION,
        "fish" => FISH_COMPLETION,
        shell => return Err(format!("unsupported shell: {shell}")),
    };
    Ok(CompletionOutcome {
        output: CompletionOutput::Script(script),
        exit_code: 0,
    })
}

const BASH_COMPLETION: &str = r#"_oqtopus()
{
  local cur prev words cword
  COMPREPLY=()
  cur="${COMP_WORDS[COMP_CWORD]}"
  prev="${COMP_WORDS[COMP_CWORD-1]}"

  case "${COMP_WORDS[1]}" in
    backend)
      case "${COMP_WORDS[2]}" in
        install)
          COMPREPLY=( $(compgen -W "engine tranqu gateway all --skip-sse-build help --help" -- "$cur") )
          return 0
          ;;
        build)
          COMPREPLY=( $(compgen -W "sse-runtime help --help" -- "$cur") )
          return 0
          ;;
        versions)
          COMPREPLY=( $(compgen -W "engine tranqu gateway help --help" -- "$cur") )
          return 0
          ;;
        update|uninstall)
          COMPREPLY=( $(compgen -W "engine tranqu gateway help --help" -- "$cur") )
          return 0
          ;;
        start)
          COMPREPLY=( $(compgen -W "core sse_engine mitigator estimator combiner tranqu gateway all --foreground help --help" -- "$cur") )
          return 0
          ;;
        stop|restart)
          COMPREPLY=( $(compgen -W "core sse_engine mitigator estimator combiner tranqu gateway all help --help" -- "$cur") )
          return 0
          ;;
        device-status)
          COMPREPLY=( $(compgen -W "show active inactive maintenance help --help" -- "$cur") )
          return 0
          ;;
        *)
          COMPREPLY=( $(compgen -W "install build versions uninstall update start stop restart status device-status info help --help" -- "$cur") )
          return 0
          ;;
      esac
      ;;
    cloud-local)
      case "${COMP_WORDS[2]}" in
        install|uninstall|update|versions)
          COMPREPLY=( $(compgen -W "cloud frontend admin all help --help" -- "$cur") )
          return 0
          ;;
        start|stop|restart)
          COMPREPLY=( $(compgen -W "db user provider admin user_signup worker all --foreground help --help" -- "$cur") )
          return 0
          ;;
        *)
          COMPREPLY=( $(compgen -W "versions install uninstall update start stop restart status info help --help" -- "$cur") )
          return 0
          ;;
      esac
      ;;
    manager)
      case "${COMP_WORDS[2]}" in
        install|versions|update|uninstall)
          COMPREPLY=( $(compgen -W "help --help" -- "$cur") )
          return 0
          ;;
        start)
          COMPREPLY=( $(compgen -W "--foreground help --help" -- "$cur") )
          return 0
          ;;
        stop|restart)
          COMPREPLY=( $(compgen -W "help --help" -- "$cur") )
          return 0
          ;;
        *)
          COMPREPLY=( $(compgen -W "install versions uninstall update start stop restart status info help --help" -- "$cur") )
          return 0
          ;;
      esac
      ;;
    init)
      COMPREPLY=( $(compgen -W "--template --branch backend cloud-local manager help --help" -- "$cur") )
      return 0
      ;;
    completion)
      COMPREPLY=( $(compgen -W "bash zsh fish" -- "$cur") )
      return 0
      ;;
    *)
      COMPREPLY=( $(compgen -W "init backend cloud-local manager completion version help --help --version" -- "$cur") )
      return 0
      ;;
  esac
}
complete -F _oqtopus oqtopus
"#;

const ZSH_COMPLETION: &str = r#"#compdef oqtopus

_oqtopus() {
  local -a commands backend_commands cloud_local_commands install_components build_targets update_components start_services services device_status cloud_local_components cloud_local_services manager_commands manager_help_only manager_start_opts shells templates
  commands=(init backend cloud-local manager completion version help --help --version)
  backend_commands=(install build versions uninstall update start stop restart status device-status info help --help)
  cloud_local_commands=(versions install uninstall update start stop restart status info help --help)
  install_components=(engine tranqu gateway all --skip-sse-build help --help)
  build_targets=(sse-runtime help --help)
  update_components=(engine tranqu gateway help --help)
  start_services=(core sse_engine mitigator estimator combiner tranqu gateway all --foreground help --help)
  services=(core sse_engine mitigator estimator combiner tranqu gateway all help --help)
  device_status=(show active inactive maintenance help --help)
  cloud_local_components=(cloud frontend admin all help --help)
  cloud_local_services=(db user provider admin user_signup worker all --foreground help --help)
  manager_commands=(install versions uninstall update start stop restart status info help --help)
  manager_help_only=(help --help)
  manager_start_opts=(--foreground help --help)
  shells=(bash zsh fish)
  templates=(backend cloud-local manager)

  case $words[2] in
    backend)
      case $words[3] in
        install) compadd -- $install_components ;;
        build) compadd -- $build_targets ;;
        versions|update|uninstall) compadd -- $update_components ;;
        start) compadd -- $start_services ;;
        stop|restart) compadd -- $services ;;
        device-status) compadd -- $device_status ;;
        *) compadd -- $backend_commands ;;
      esac
      ;;
    cloud-local)
      case $words[3] in
        install|uninstall|update|versions) compadd -- $cloud_local_components ;;
        start|stop|restart) compadd -- $cloud_local_services ;;
        *) compadd -- $cloud_local_commands ;;
      esac
      ;;
    manager)
      case $words[3] in
        install|versions|update|uninstall|stop|restart) compadd -- $manager_help_only ;;
        start) compadd -- $manager_start_opts ;;
        *) compadd -- $manager_commands ;;
      esac
      ;;
    init) compadd -- --template --branch help --help $templates ;;
    completion) compadd -- $shells ;;
    *) compadd -- $commands ;;
  esac
}

_oqtopus "$@"
"#;

const FISH_COMPLETION: &str = r#"complete -c oqtopus -f -n "__fish_use_subcommand" -a "init backend cloud-local manager completion version help --help --version"
complete -c oqtopus -f -n "__fish_seen_subcommand_from completion" -a "bash zsh fish"
complete -c oqtopus -f -n "__fish_seen_subcommand_from init" -a "--template --branch backend cloud-local manager help --help"
complete -c oqtopus -f -n "__fish_seen_subcommand_from backend" -a "install build versions uninstall update start stop restart status device-status info help --help"
complete -c oqtopus -f -n "__fish_seen_subcommand_from backend; and __fish_seen_subcommand_from install" -a "engine tranqu gateway all --skip-sse-build help --help"
complete -c oqtopus -f -n "__fish_seen_subcommand_from backend; and __fish_seen_subcommand_from build" -a "sse-runtime help --help"
complete -c oqtopus -f -n "__fish_seen_subcommand_from backend; and __fish_seen_subcommand_from versions update uninstall" -a "engine tranqu gateway help --help"
complete -c oqtopus -f -n "__fish_seen_subcommand_from backend; and __fish_seen_subcommand_from start" -a "core sse_engine mitigator estimator combiner tranqu gateway all --foreground help --help"
complete -c oqtopus -f -n "__fish_seen_subcommand_from backend; and __fish_seen_subcommand_from stop restart" -a "core sse_engine mitigator estimator combiner tranqu gateway all help --help"
complete -c oqtopus -f -n "__fish_seen_subcommand_from backend; and __fish_seen_subcommand_from device-status" -a "show active inactive maintenance help --help"
complete -c oqtopus -f -n "__fish_seen_subcommand_from cloud-local" -a "versions install uninstall update start stop restart status info help --help"
complete -c oqtopus -f -n "__fish_seen_subcommand_from cloud-local; and __fish_seen_subcommand_from install uninstall update versions" -a "cloud frontend admin all help --help"
complete -c oqtopus -f -n "__fish_seen_subcommand_from cloud-local; and __fish_seen_subcommand_from start stop restart" -a "db user provider admin user_signup worker all --foreground help --help"
complete -c oqtopus -f -n "__fish_seen_subcommand_from manager" -a "install versions uninstall update start stop restart status info help --help"
complete -c oqtopus -f -n "__fish_seen_subcommand_from manager; and __fish_seen_subcommand_from start" -a "--foreground help --help"
"#;
