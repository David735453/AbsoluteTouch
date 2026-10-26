//! Command-line interface.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Args, Parser, Subcommand};

use crate::config::{Config, Overrides, TransportMode, default_path, encode_hex16, generate_token};
use crate::error::Error;
use crate::inject::backend;
use crate::run;

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
    pub config: Option<PathBuf>,
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
    pub record: Option<PathBuf>,
}

/// Options for `replay`.
#[derive(Debug, Args)]
pub struct ReplayArgs {
    /// Trace file to replay.
    #[arg(value_name = "FILE")]
    pub file: PathBuf,
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
/// Returns [`Error`] when the selected command fails.
pub fn run() -> Result<(), Error> {
    dispatch(Cli::parse())
}

fn dispatch(cli: Cli) -> Result<(), Error> {
    let config_path = cli.config.clone();
    match cli.command.unwrap_or(Command::Doctor) {
        Command::Doctor => {
            crate::doctor::report();
            Ok(())
        }
        Command::Token(args) => token(&args, config_path.as_deref()),
        Command::Devices => Err(Error::NotImplemented { feature: "devices" }),
        Command::Run(args) => run_session(args, config_path.as_deref()),
        Command::Replay(args) => replay_trace(&args, config_path.as_deref()),
    }
}

fn run_session(args: RunArgs, config_path: Option<&Path>) -> Result<(), Error> {
    let overrides = Overrides {
        transport: args.transport.map(Into::into),
        backend: args.backend,
        dry_run: args.dry_run.then_some(true),
    };
    let config = Config::load(config_path)?
        .apply_overrides(&overrides)
        .validate()?;

    if config.transport == TransportMode::Usb {
        return Err(Error::NotImplemented {
            feature: "usb transport",
        });
    }

    let mut injector = backend(&config.backend, config.dry_run)?;
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst))
        .map_err(|error| Error::Signal(error.to_string()))?;

    let report = run::wifi(&config, injector.as_mut(), args.record.as_deref(), &stop)?;

    println!(
        "injected {} samples, recorded {} packets",
        report.injected, report.recorded
    );
    eprintln!("{}", report.stats);
    Ok(())
}

fn replay_trace(args: &ReplayArgs, config_path: Option<&Path>) -> Result<(), Error> {
    let overrides = Overrides {
        dry_run: Some(args.dry_run),
        ..Overrides::default()
    };
    let config = Config::load(config_path)?
        .apply_overrides(&overrides)
        .validate()?;
    let mut injector = backend(&config.backend, true)?;

    let report = run::replay(
        &args.file,
        args.speed,
        args.dry_run,
        &config,
        injector.as_mut(),
    )?;

    println!(
        "replayed {} records, {} samples",
        report.records, report.stats.samples
    );
    eprintln!("{}", report.stats);
    Ok(())
}

fn token(args: &TokenArgs, config_path: Option<&Path>) -> Result<(), Error> {
    let path = config_path.map(PathBuf::from).or_else(default_path);

    let mut config = match &path {
        Some(path) if path.exists() => Config::from_file(path)?,
        _ => Config::default(),
    };

    if args.reset {
        let new_token = encode_hex16(&generate_token());
        config.token = Some(new_token.clone());
        let config = config.validate()?;
        if let Some(path) = &path {
            config.save(path)?;
            eprintln!("wrote {}", path.display());
        }
        println!("{new_token}");
        return Ok(());
    }

    match &config.token {
        Some(token) => println!("{token}"),
        None => {
            eprintln!(
                "no token configured{}; run `absolutetouch token --reset`",
                path.map(|path| format!(" in {}", path.display()))
                    .unwrap_or_default()
            );
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use clap::Parser;

    use super::{Cli, Command, TransportArg, dispatch};
    use crate::config::Config;

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

    #[test]
    fn token_reset_writes_and_reads_the_file() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("config.toml");
        let cli = Cli::try_parse_from([
            "absolutetouch",
            "--config",
            path.to_str().expect("path"),
            "token",
            "--reset",
        ])
        .expect("parse token");

        dispatch(cli).expect("reset token");

        let config = Config::from_file(&path).expect("load");
        let token = config.token.expect("token");
        assert_eq!(token.len(), 32);
        assert_eq!(
            token,
            crate::config::encode_hex16(&crate::config::decode_hex16(&token).expect("decode"))
        );
    }

    #[test]
    fn devices_reports_not_implemented() {
        let cli = Cli::try_parse_from(["absolutetouch", "devices"]).expect("parse devices");

        assert!(matches!(
            dispatch(cli),
            Err(crate::error::Error::NotImplemented { .. })
        ));
    }

    #[test]
    fn usb_run_reports_not_implemented() {
        let cli =
            Cli::try_parse_from(["absolutetouch", "run", "--transport", "usb"]).expect("parse run");

        assert!(matches!(
            dispatch(cli),
            Err(crate::error::Error::NotImplemented { .. })
        ));
    }
}
