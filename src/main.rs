//! OQTOPUS command-line entry point.
//!
//! Commands are being migrated incrementally from the legacy Bash implementation. This crate
//! handles migrated routes directly and replaces itself with the legacy CLI for all other routes,
//! preserving command-line compatibility during the transition.

mod archive;
mod backend;
mod cli;
mod cloud_local;
mod environment;
mod init;
mod legacy;
mod manager;
mod metadata;
mod operations;
mod progress;
mod remote;
mod service;
mod text;
mod version;
mod versions;

use std::env;
use std::io;
use std::process;

use backend::{backend_device_status, backend_info, backend_status};
use cli::{Route, route};
use cloud_local::{cloud_local_info, cloud_local_status};
use init::init;
use legacy::run_legacy;
use manager::{manager_info, manager_status};
use operations::{
    backend_build, backend_install, backend_uninstall, backend_update, cloud_local_install,
    cloud_local_uninstall, cloud_local_update, manager_install, manager_uninstall, manager_update,
};
use version::version_info;
use versions::{backend_versions, cloud_local_versions, manager_versions};

const EXIT_SUCCESS: i32 = 0;
const EXIT_FAILURE: i32 = 1;

fn main() {
    // Rust ignores SIGPIPE by default. CLI pipelines expect the traditional Unix behavior: exit
    // silently when a downstream reader such as `head` or `grep -q` closes the pipe early.
    // SAFETY: installing the default disposition for SIGPIPE requires no Rust-managed callback.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let args: Vec<String> = env::args().skip(1).collect();
    // Template subcommands consume two leading words. `init` consumes only its top-level word.
    let command_args = args.get(2..).unwrap_or_default();
    let init_args = args.get(1..).unwrap_or_default();

    // Commands report their own exit status; a returned message is always a failure.
    //
    // Most commands compute a result and then render it, so they take the stdout lock only for the
    // rendering step. The install, build, uninstall, and update routes instead stream progress
    // while they work, so they hold one lock across both the command and its final output; that
    // keeps progress and result in a single ordered stream.
    let outcome: Result<i32, String> = match route(&args) {
        Route::Help => text::write_help(&mut io::stdout().lock())
            .map(|()| EXIT_SUCCESS)
            .map_err(|error| format!("failed to write help: {error}")),
        Route::Version => text::write_version(&mut io::stdout().lock(), &version_info())
            .map(|()| EXIT_SUCCESS)
            .map_err(|error| format!("failed to write version: {error}")),
        Route::Init => init(init_args).and_then(|result| {
            text::write_init(&mut io::stdout().lock(), &result)
                .map(|()| result.exit_code())
                .map_err(|error| format!("failed to write init result: {error}"))
        }),
        Route::BackendInfo => backend_info(command_args).and_then(|info| {
            text::write_backend_info(&mut io::stdout().lock(), &info)
                .map(|()| EXIT_SUCCESS)
                .map_err(|error| format!("failed to write backend info: {error}"))
        }),
        Route::BackendStatus => backend_status(command_args).and_then(|status| {
            text::write_backend_status(&mut io::stdout().lock(), &status)
                .map(|()| EXIT_SUCCESS)
                .map_err(|error| format!("failed to write backend status: {error}"))
        }),
        Route::BackendDeviceStatus => backend_device_status(command_args).and_then(|status| {
            text::write_backend_device_status(&mut io::stdout().lock(), &status)
                .map(|()| status.exit_code())
                .map_err(|error| format!("failed to write backend device status: {error}"))
        }),
        Route::BackendVersions => backend_versions(command_args).and_then(|result| {
            text::write_versions(&mut io::stdout().lock(), &result)
                .map(|()| result.exit_code())
                .map_err(|error| format!("failed to write backend versions: {error}"))
        }),
        Route::BackendInstall => {
            let mut stdout = io::stdout().lock();
            backend_install(command_args, &mut stdout).and_then(|result| {
                text::write_operation(&mut stdout, &result)
                    .map(|()| result.exit_code())
                    .map_err(|error| format!("failed to write backend install result: {error}"))
            })
        }
        Route::BackendBuild => {
            let mut stdout = io::stdout().lock();
            backend_build(command_args, &mut stdout).and_then(|result| {
                text::write_operation(&mut stdout, &result)
                    .map(|()| result.exit_code())
                    .map_err(|error| format!("failed to write backend build result: {error}"))
            })
        }
        Route::BackendUninstall => {
            let mut stdout = io::stdout().lock();
            backend_uninstall(command_args, &mut stdout).and_then(|result| {
                text::write_operation(&mut stdout, &result)
                    .map(|()| result.exit_code())
                    .map_err(|error| format!("failed to write backend uninstall result: {error}"))
            })
        }
        Route::BackendUpdate => {
            let mut stdout = io::stdout().lock();
            backend_update(command_args, &mut stdout).and_then(|result| {
                text::write_operation(&mut stdout, &result)
                    .map(|()| result.exit_code())
                    .map_err(|error| format!("failed to write backend update result: {error}"))
            })
        }
        Route::CloudLocalInfo => cloud_local_info(command_args).and_then(|info| {
            text::write_cloud_local_info(&mut io::stdout().lock(), &info)
                .map(|()| EXIT_SUCCESS)
                .map_err(|error| format!("failed to write cloud-local info: {error}"))
        }),
        Route::CloudLocalStatus => cloud_local_status(command_args).and_then(|status| {
            text::write_cloud_local_status(&mut io::stdout().lock(), &status)
                .map(|()| EXIT_SUCCESS)
                .map_err(|error| format!("failed to write cloud-local status: {error}"))
        }),
        Route::CloudLocalVersions => cloud_local_versions(command_args).and_then(|result| {
            text::write_versions(&mut io::stdout().lock(), &result)
                .map(|()| result.exit_code())
                .map_err(|error| format!("failed to write cloud-local versions: {error}"))
        }),
        Route::CloudLocalInstall => {
            let mut stdout = io::stdout().lock();
            cloud_local_install(command_args, &mut stdout).and_then(|result| {
                text::write_operation(&mut stdout, &result)
                    .map(|()| result.exit_code())
                    .map_err(|error| format!("failed to write cloud-local install result: {error}"))
            })
        }
        Route::CloudLocalUninstall => {
            let mut stdout = io::stdout().lock();
            cloud_local_uninstall(command_args, &mut stdout).and_then(|result| {
                text::write_operation(&mut stdout, &result)
                    .map(|()| result.exit_code())
                    .map_err(|error| {
                        format!("failed to write cloud-local uninstall result: {error}")
                    })
            })
        }
        Route::CloudLocalUpdate => {
            let mut stdout = io::stdout().lock();
            cloud_local_update(command_args, &mut stdout).and_then(|result| {
                text::write_operation(&mut stdout, &result)
                    .map(|()| result.exit_code())
                    .map_err(|error| format!("failed to write cloud-local update result: {error}"))
            })
        }
        Route::ManagerInfo => manager_info(command_args).and_then(|info| {
            text::write_manager_info(&mut io::stdout().lock(), &info)
                .map(|()| EXIT_SUCCESS)
                .map_err(|error| format!("failed to write manager info: {error}"))
        }),
        Route::ManagerStatus => manager_status(command_args).and_then(|status| {
            text::write_manager_status(&mut io::stdout().lock(), &status)
                .map(|()| EXIT_SUCCESS)
                .map_err(|error| format!("failed to write manager status: {error}"))
        }),
        Route::ManagerVersions => manager_versions(command_args).and_then(|result| {
            text::write_versions(&mut io::stdout().lock(), &result)
                .map(|()| result.exit_code())
                .map_err(|error| format!("failed to write manager versions: {error}"))
        }),
        Route::ManagerInstall => {
            let mut stdout = io::stdout().lock();
            manager_install(command_args, &mut stdout).and_then(|result| {
                text::write_operation(&mut stdout, &result)
                    .map(|()| result.exit_code())
                    .map_err(|error| format!("failed to write manager install result: {error}"))
            })
        }
        Route::ManagerUninstall => {
            let mut stdout = io::stdout().lock();
            manager_uninstall(command_args, &mut stdout).and_then(|result| {
                text::write_operation(&mut stdout, &result)
                    .map(|()| result.exit_code())
                    .map_err(|error| format!("failed to write manager uninstall result: {error}"))
            })
        }
        Route::ManagerUpdate => {
            let mut stdout = io::stdout().lock();
            manager_update(command_args, &mut stdout).and_then(|result| {
                text::write_operation(&mut stdout, &result)
                    .map(|()| result.exit_code())
                    .map_err(|error| format!("failed to write manager update result: {error}"))
            })
        }
        Route::Legacy => run_legacy(&args),
    };

    match outcome {
        Ok(EXIT_SUCCESS) => {}
        Ok(code) => process::exit(code),
        Err(error) => {
            let _ = text::write_error(&mut io::stderr().lock(), &error);
            process::exit(EXIT_FAILURE);
        }
    }
}
