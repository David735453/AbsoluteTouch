//! Command-line interface.

use clap::{Args, Parser, Subcommand};

use crate::config::TransportMode;
use crate::error::Error;

/// Command-line options for AbsoluteTouch.
#[derive(Debug, Parser)]
#[command(
    name = "absolutetouch",
    version,
    about = "Stream iPad tablet input to a desktop"
)]
pub struct Cli {
    /// Configuration file path.
    #[arg(long, global = true, value_name = "PATH")]
    pub config: Option<std::path::PathBuf>,
    /// Subcommand to run. Defaults to `doctor`.
    #[command(subcommand)]
    pub command: Option<Command>,
}

/// Top-level commands.
#[derive(Debug, Subcommand)]
pub enum Command {
    /// Stream tablet input from an iPad.
    Run(RunArgs),
    /// Replay a recorded trace.
    Replay(ReplayArgs),
    /// List USB devices visible through usbmuxd.
    Devices,
    /// Check permissions and backend availability.
    Doctor,
    /// Show or reset the pairing token.
    Token(TokenArgs),
}

/// Options for `run`.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// Transport to use.
    #[arg(long, value_enum)]
    pub transport: Option<TransportArg>,
    /// Injection backend; `auto` selects the best available.
    #[arg(long, value_name = "BACKEND")]
    pub backend: Option<String>,
    /// Print events without injecting them.
    #[arg(long)]
    pub dry_run: bool,
    /// Record packets to a trace file.
    #[arg(long, value_name = "FILE")]
    pub record: Option<std::path::PathBuf>,
}

/// Options for `replay`.
#[derive(Debug, Args)]
pub struct ReplayArgs {
    /// Trace file to replay.
    #[arg(value_name = "FILE")]
    pub file: std::path::PathBuf,
    /// Print events without injecting them.
    #[arg(long)]
    pub dry_run: bool,
    /// Playback speed multiplier.
    #[arg(long, default_value_t = 1.0)]
    pub speed: f32,
}

/// Options for `token`.
#[derive(Debug, Args)]
pub struct TokenArgs {
    /// Generate a new token.
    #[arg(long)]
    pub reset: bool,
}

/// Transport selection accepted by the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum TransportArg {
    /// Wi-Fi UDP.
    Wifi,
    /// USB via usbmuxd.
    Usb,
}

impl From<TransportArg> for TransportMode {
    fn from(value: TransportArg) -> Self {
        match value {
            TransportArg::Wifi => Self::Wifi,
            TransportArg::Usb => Self::Usb,
        }
    }
}

/// Parse arguments and execute the selected command.
///
/// # Errors
///
/// Returns [`Error`] when the selected command fails or is not implemented in
/// the current milestone.
pub fn run() -> Result<(), Error> {
    let cli = Cli::parse();
    let command = cli.command.unwrap_or(Command::Doctor);

    if matches!(&command, Command::Doctor) {
        crate::doctor::report();
        return Ok(());
    }

    Err(Error::NotImplemented {
        feature: command.name(),
    })
}

impl Command {
    const fn name(&self) -> &'static str {
        match self {
            Self::Run(_) => "run",
            Self::Replay(_) => "replay",
            Self::Devices => "devices",
            Self::Doctor => "doctor",
            Self::Token(_) => "token",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Cli, Command, TransportArg};
    use clap::Parser;

    #[test]
    fn parses_doctor() {
        let cli = Cli::try_parse_from(["absolutetouch", "doctor"]).expect("parse doctor");

        assert!(matches!(cli.command, Some(Command::Doctor)));
    }

    #[test]
    fn defaults_to_doctor() {
        let cli = Cli::try_parse_from(["absolutetouch"]).expect("parse default");

        assert!(cli.command.is_none());
    }

    #[test]
    fn parses_run_options() {
        let cli = Cli::try_parse_from(["absolutetouch", "run", "--transport", "usb", "--dry-run"])
            .expect("parse run");

        match cli.command {
            Some(Command::Run(args)) => {
                assert_eq!(args.transport, Some(TransportArg::Usb));
                assert!(args.dry_run);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn parses_replay_options() {
        let cli = Cli::try_parse_from(["absolutetouch", "replay", "trace.bin", "--speed", "2.0"])
            .expect("parse replay");

        match cli.command {
            Some(Command::Replay(args)) => {
                assert_eq!(args.file, std::path::PathBuf::from("trace.bin"));
                assert!((args.speed - 2.0).abs() < f32::EPSILON);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
