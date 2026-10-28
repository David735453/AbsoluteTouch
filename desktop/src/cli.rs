//! Command-line interface.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use clap::{Args, Parser, Subcommand};

use crate::config::{Config, Overrides, TransportMode, default_path, encode_hex16, generate_token};
use crate::error::Error;
use crate::inject::{DeviceMode, backend};
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
    /// Show or reset the pairing code.
    Token(TokenArgs),
}

/// Options for `run`.
#[derive(Debug, Args)]
pub struct RunArgs {
    /// Transport to use.
    #[arg(long, value_enum)]
    pub transport: Option<TransportArg>,
    /// USB device serial, or a fragment of it.
    #[arg(long, value_name = "SERIAL")]
    pub serial: Option<String>,
    /// Injection backend; `auto` selects the best available.
    #[arg(long, value_name = "BACKEND")]
    pub backend: Option<String>,
    /// Virtual device to register; `pointer` moves the desktop cursor.
    #[arg(long, value_enum)]
    pub device: Option<DeviceArg>,
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
    /// Virtual device to register; overrides the configuration.
    #[arg(long, value_enum)]
    pub device: Option<DeviceArg>,
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
    /// Generate a new pairing code.
    #[arg(long)]
    pub reset: bool,
    /// Generate a 32-character token instead of a 6-character code.
    #[arg(long, requires = "reset")]
    pub hex: bool,
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

/// Virtual device selection accepted by the CLI.
#[derive(Debug, Clone, Copy, PartialEq, Eq, clap::ValueEnum)]
pub enum DeviceArg {
    /// Absolute pointer that moves the desktop cursor.
    Pointer,
    /// Stylus tablet with tool, pressure, and tilt events.
    Tablet,
}

impl From<DeviceArg> for DeviceMode {
    fn from(value: DeviceArg) -> Self {
        match value {
            DeviceArg::Pointer => Self::Pointer,
            DeviceArg::Tablet => Self::Tablet,
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
        Command::Devices => devices(),
        Command::Run(args) => run_session(args, config_path.as_deref()),
        Command::Replay(args) => replay_trace(&args, config_path.as_deref()),
    }
}

fn run_session(args: RunArgs, config_path: Option<&Path>) -> Result<(), Error> {
    let overrides = Overrides {
        transport: args.transport.map(Into::into),
        backend: args.backend,
        device: args.device.map(Into::into),
        dry_run: args.dry_run.then_some(true),
    };
    let config = Config::load(config_path)?
        .apply_overrides(&overrides)
        .validate()?;

    let mut injector = backend(&config.backend, config.dry_run, config.device)?;
    let stop = Arc::new(AtomicBool::new(false));
    let flag = Arc::clone(&stop);
    ctrlc::set_handler(move || flag.store(true, Ordering::SeqCst))
        .map_err(|error| Error::Signal(error.to_string()))?;

    let report = match config.transport {
        TransportMode::Wifi => {
            run::wifi(&config, injector.as_mut(), args.record.as_deref(), &stop)?
        }
        TransportMode::Usb => run::usb(
            &config,
            args.serial.as_deref(),
            injector.as_mut(),
            args.record.as_deref(),
            &stop,
        )?,
    };

    println!(
        "injected {} samples, recorded {} packets",
        report.injected, report.recorded
    );
    eprintln!("{}", report.stats);
    Ok(())
}

/// Print the USB devices `usbmuxd` reports.
fn devices() -> Result<(), Error> {
    let mut daemon = crate::transport::usbmux::Daemon::connect()?;
    let devices = daemon.list_devices()?;

    if devices.is_empty() {
        println!("no devices attached");
        return Ok(());
    }

    for device in devices {
        println!(
            "{}\tdevice {}\tproduct {:#06x}\t{}",
            device.serial, device.id, device.product_id, device.connection_type
        );
    }
    Ok(())
}

fn replay_trace(args: &ReplayArgs, config_path: Option<&Path>) -> Result<(), Error> {
    let overrides = Overrides {
        device: args.device.map(Into::into),
        dry_run: args.dry_run.then_some(true),
        ..Overrides::default()
    };
    let config = Config::load(config_path)?
        .apply_overrides(&overrides)
        .validate()?;
    let mut injector = backend(&config.backend, config.dry_run, config.device)?;

    let report = run::replay(
        &args.file,
        args.speed,
        config.dry_run,
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
        let new_token = if args.hex {
            encode_hex16(&generate_token())
        } else {
            crate::pairing::generate_code()
        };
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
                "no pairing code configured{}; run `absolutetouch token --reset`",
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

    use super::{Cli, Command, DeviceArg, TransportArg, dispatch};
    use crate::config::Config;
    use crate::inject::DeviceMode;

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
        let cli = Cli::try_parse_from([
            "absolutetouch",
            "run",
            "--transport",
            "usb",
            "--device",
            "tablet",
            "--dry-run",
        ])
        .expect("parse run");

        match cli.command {
            Some(Command::Run(args)) => {
                assert_eq!(args.transport, Some(TransportArg::Usb));
                assert_eq!(args.device, Some(DeviceArg::Tablet));
                assert!(args.dry_run);
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn device_arguments_map_to_the_backend() {
        assert_eq!(DeviceMode::from(DeviceArg::Pointer), DeviceMode::Pointer);
        assert_eq!(DeviceMode::from(DeviceArg::Tablet), DeviceMode::Tablet);
    }

    #[test]
    fn unknown_device_name_is_rejected_by_the_parser() {
        assert!(Cli::try_parse_from(["absolutetouch", "run", "--device", "pen"]).is_err());
    }

    #[test]
    fn parses_replay_options() {
        let cli = Cli::try_parse_from([
            "absolutetouch",
            "replay",
            "trace.bin",
            "--speed",
            "2.0",
            "--device",
            "pointer",
        ])
        .expect("parse replay");

        match cli.command {
            Some(Command::Replay(args)) => {
                assert_eq!(args.file, std::path::PathBuf::from("trace.bin"));
                assert!((args.speed - 2.0).abs() < f32::EPSILON);
                assert_eq!(args.device, Some(DeviceArg::Pointer));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }

    #[test]
    fn token_reset_writes_a_code_that_derives_the_configured_token() {
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
        let code = config.token.clone().expect("token");
        assert_eq!(code.len(), crate::pairing::CODE_LENGTH);
        assert_eq!(crate::pairing::normalize(&code).expect("normalize"), code);
        assert_eq!(
            config.token_bytes().expect("token bytes"),
            Some(crate::pairing::token_from_code(&code).expect("derive"))
        );
    }

    #[test]
    fn token_reset_hex_writes_a_full_token() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("config.toml");
        let cli = Cli::try_parse_from([
            "absolutetouch",
            "--config",
            path.to_str().expect("path"),
            "token",
            "--reset",
            "--hex",
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
    fn parses_the_usb_serial_selector() {
        let cli = Cli::try_parse_from([
            "absolutetouch",
            "run",
            "--transport",
            "usb",
            "--serial",
            "00008110",
        ])
        .expect("parse run");

        match cli.command {
            Some(Command::Run(args)) => {
                assert_eq!(args.transport, Some(TransportArg::Usb));
                assert_eq!(args.serial.as_deref(), Some("00008110"));
            }
            other => panic!("unexpected command: {other:?}"),
        }
    }
}
