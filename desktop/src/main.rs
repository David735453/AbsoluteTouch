//! Command-line entry point.

use std::process::ExitCode;

fn main() -> ExitCode {
    match absolutetouch::cli::run() {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("error: {error}");
            ExitCode::FAILURE
        }
    }
}
