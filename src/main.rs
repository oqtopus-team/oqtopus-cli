//! OQTOPUS command-line entry point: signal setup, UTF-8 argument validation, and process exit.

mod archive;
mod args;
mod backend;
mod cli;
mod cloud_local;
mod completion;
mod environment;
mod init;
mod lifecycle;
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
use std::process;

fn main() {
    // Rust ignores SIGPIPE by default. CLI pipelines expect the traditional Unix behavior: exit
    // silently when a downstream reader such as `head` or `grep -q` closes the pipe early.
    // SAFETY: installing the default disposition for SIGPIPE requires no Rust-managed callback.
    unsafe {
        libc::signal(libc::SIGPIPE, libc::SIG_DFL);
    }

    let args: Result<Vec<String>, _> = env::args_os()
        .skip(1)
        .map(|arg| arg.into_string())
        .collect();
    let args = match args {
        Ok(args) => args,
        Err(_) => {
            let _ = text::write_error(
                &mut std::io::stderr().lock(),
                "command-line arguments must be valid UTF-8.",
            );
            process::exit(1);
        }
    };
    let exit_code = cli::run(&args);
    if exit_code != 0 {
        process::exit(exit_code);
    }
}
