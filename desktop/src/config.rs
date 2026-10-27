//! Configuration loading and precedence.
//!
//! Precedence is CLI overrides, then `ABSOLUTETOUCH_*` environment variables,
//! then the TOML file, then the defaults in [`Config::default`].

use std::net::IpAddr;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use thiserror::Error;

use crate::inject::DeviceMode;
use crate::mapping::{AspectMode, Rect};

/// Transport selected for a session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum TransportMode {
    /// Wi-Fi UDP.
    #[default]
    Wifi,
    /// USB via usbmuxd.
    Usb,
}

/// Top-level configuration.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct Config {
    /// Transport to use.
    pub transport: TransportMode,
    /// Address the UDP listener binds to.
    pub bind: String,
    /// UDP port used by the Wi-Fi transport.
    pub udp_port: u16,
    /// TCP port used by the USB transport.
    pub tcp_port: u16,
    /// Optional pairing token, as 32 hexadecimal characters.
    pub token: Option<String>,
    /// Milliseconds without a valid packet before input is released.
    pub watchdog_ms: u64,
    /// Milliseconds without a valid packet before the session ends.
    pub session_timeout_ms: u64,
    /// Pressure substituted when the iPad does not report it.
    pub pressure_default: u16,
    /// Decode and log without injecting.
    pub dry_run: bool,
    /// Injection backend: `auto`, `mock`, `uinput`, `windows`, or `macos`.
    pub backend: String,
    /// Virtual device to register: `pointer` or `tablet`.
    pub device: DeviceMode,
    /// Coordinate mapping settings.
    pub mapping: MappingConfig,
}

/// Coordinate mapping settings.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(default, deny_unknown_fields)]
pub struct MappingConfig {
    /// Target rectangle in normalized desktop coordinates.
    pub target: Rect,
    /// Aspect behavior when both aspect ratios are known.
    pub aspect: AspectMode,
    /// Width divided by height of the iPad active area.
    pub source_aspect: Option<f64>,
    /// Width divided by height of the desktop target.
    pub target_aspect: Option<f64>,
}

impl Default for MappingConfig {
    fn default() -> Self {
        Self {
            target: Rect::FULL,
            aspect: AspectMode::Preserve,
            source_aspect: Some(4.0 / 3.0),
            target_aspect: None,
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self {
            transport: TransportMode::Wifi,
            bind: "0.0.0.0".to_owned(),
            udp_port: 37_689,
            tcp_port: 37_690,
            token: None,
            watchdog_ms: 750,
            session_timeout_ms: 5_000,
            pressure_default: 32_768,
            dry_run: false,
            backend: "auto".to_owned(),
            device: DeviceMode::Pointer,
            mapping: MappingConfig::default(),
        }
    }
}

/// Command-line overrides applied after the file and environment layers.
#[derive(Debug, Clone, Default)]
pub struct Overrides {
    /// Transport selected on the command line.
    pub transport: Option<TransportMode>,
    /// Backend selected on the command line.
    pub backend: Option<String>,
    /// Virtual device selected on the command line.
    pub device: Option<DeviceMode>,
    /// Dry-run selected on the command line.
    pub dry_run: Option<bool>,
}

/// Configuration errors.
#[derive(Debug, Error)]
pub enum ConfigError {
    /// The configuration file could not be read.
    #[error("cannot read config {path}: {source}")]
    Read {
        /// Path that failed.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The configuration file is not valid TOML for this schema.
    #[error("invalid config {path}: {source}")]
    Parse {
        /// Path that failed.
        path: PathBuf,
        /// Underlying TOML error.
        #[source]
        source: toml::de::Error,
    },
    /// A value is syntactically valid but not usable.
    #[error("invalid configuration: {0}")]
    Invalid(String),
    /// An environment variable could not be parsed.
    #[error("invalid value for {name}: {value}")]
    InvalidValue {
        /// Environment variable name.
        name: &'static str,
        /// Rejected value.
        value: String,
    },
    /// The configuration file could not be written.
    #[error("cannot write config {path}: {source}")]
    Write {
        /// Path that failed.
        path: PathBuf,
        /// Underlying I/O error.
        #[source]
        source: std::io::Error,
    },
    /// The configuration could not be serialized.
    #[error("cannot serialize config: {0}")]
    Serialize(String),
}

impl Config {
    /// Load the configuration from an optional path plus the environment.
    ///
    /// When no path is given, the platform configuration directory is used if
    /// the file exists. A missing default file is not an error.
    pub fn load(path: Option<&Path>) -> Result<Self, ConfigError> {
        let config = match path {
            Some(path) => Self::from_file(path)?,
            None => match default_path() {
                Some(path) if path.exists() => Self::from_file(&path)?,
                _ => Self::default(),
            },
        };

        config.apply_env()?.validate()
    }

    /// Read a configuration file, filling missing fields with defaults.
    pub fn from_file(path: &Path) -> Result<Self, ConfigError> {
        let text = std::fs::read_to_string(path).map_err(|source| ConfigError::Read {
            path: path.to_path_buf(),
            source,
        })?;
        toml::from_str(&text).map_err(|source| ConfigError::Parse {
            path: path.to_path_buf(),
            source,
        })
    }

    /// Apply `ABSOLUTETOUCH_*` environment variables.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::InvalidValue`] when a variable cannot be parsed.
    pub fn apply_env(self) -> Result<Self, ConfigError> {
        self.apply_env_with(|name| std::env::var(name).ok())
    }

    fn apply_env_with(self, lookup: impl Fn(&str) -> Option<String>) -> Result<Self, ConfigError> {
        let mut config = self;

        if let Some(value) = present(lookup("ABSOLUTETOUCH_BIND")) {
            config.bind = value;
        }
        if let Some(value) = present(lookup("ABSOLUTETOUCH_BACKEND")) {
            config.backend = value;
        }
        if let Some(value) = present(lookup("ABSOLUTETOUCH_DEVICE")) {
            config.device = DeviceMode::from_name(&value).ok_or(ConfigError::InvalidValue {
                name: "ABSOLUTETOUCH_DEVICE",
                value,
            })?;
        }
        if let Some(value) = present(lookup("ABSOLUTETOUCH_TOKEN")) {
            config.token = Some(value);
        }
        if let Some(value) = present(lookup("ABSOLUTETOUCH_TRANSPORT")) {
            config.transport = match value.as_str() {
                "wifi" => TransportMode::Wifi,
                "usb" => TransportMode::Usb,
                _ => {
                    return Err(ConfigError::InvalidValue {
                        name: "ABSOLUTETOUCH_TRANSPORT",
                        value,
                    });
                }
            };
        }
        if let Some(value) = present(lookup("ABSOLUTETOUCH_UDP_PORT")) {
            config.udp_port = parse(&value, "ABSOLUTETOUCH_UDP_PORT")?;
        }
        if let Some(value) = present(lookup("ABSOLUTETOUCH_TCP_PORT")) {
            config.tcp_port = parse(&value, "ABSOLUTETOUCH_TCP_PORT")?;
        }
        if let Some(value) = present(lookup("ABSOLUTETOUCH_WATCHDOG_MS")) {
            config.watchdog_ms = parse(&value, "ABSOLUTETOUCH_WATCHDOG_MS")?;
        }
        if let Some(value) = present(lookup("ABSOLUTETOUCH_SESSION_TIMEOUT_MS")) {
            config.session_timeout_ms = parse(&value, "ABSOLUTETOUCH_SESSION_TIMEOUT_MS")?;
        }
        if let Some(value) = present(lookup("ABSOLUTETOUCH_PRESSURE_DEFAULT")) {
            config.pressure_default = parse(&value, "ABSOLUTETOUCH_PRESSURE_DEFAULT")?;
        }
        if let Some(value) = present(lookup("ABSOLUTETOUCH_DRY_RUN")) {
            config.dry_run = parse(&value, "ABSOLUTETOUCH_DRY_RUN")?;
        }

        Ok(config)
    }

    /// Apply command-line overrides.
    #[must_use]
    pub fn apply_overrides(mut self, overrides: &Overrides) -> Self {
        if let Some(transport) = overrides.transport {
            self.transport = transport;
        }
        if let Some(backend) = &overrides.backend {
            self.backend.clone_from(backend);
        }
        if let Some(device) = overrides.device {
            self.device = device;
        }
        if let Some(dry_run) = overrides.dry_run {
            self.dry_run = dry_run;
        }
        self
    }

    /// Validate cross-field constraints.
    ///
    /// # Errors
    ///
    /// Returns [`ConfigError::Invalid`] when a value is out of range or a pair
    /// of values is inconsistent.
    pub fn validate(self) -> Result<Self, ConfigError> {
        if self.bind.parse::<IpAddr>().is_err() {
            return Err(ConfigError::Invalid(format!(
                "bind must be an IP address, got {:?}",
                self.bind
            )));
        }
        if self.udp_port == 0 {
            return Err(ConfigError::Invalid("udp_port must not be 0".to_owned()));
        }
        if self.tcp_port == 0 {
            return Err(ConfigError::Invalid("tcp_port must not be 0".to_owned()));
        }
        if !(50..=60_000).contains(&self.watchdog_ms) {
            return Err(ConfigError::Invalid(
                "watchdog_ms must be between 50 and 60000".to_owned(),
            ));
        }
        if self.session_timeout_ms <= self.watchdog_ms {
            return Err(ConfigError::Invalid(
                "session_timeout_ms must be greater than watchdog_ms".to_owned(),
            ));
        }
        if self.backend.is_empty() {
            return Err(ConfigError::Invalid("backend must not be empty".to_owned()));
        }
        if let Some(token) = &self.token {
            decode_hex16(token)?;
        }
        if !self.mapping.target.is_inside_full_range() {
            return Err(ConfigError::Invalid(
                "mapping.target must fit inside 0..=65535".to_owned(),
            ));
        }
        if let Some(aspect) = self.mapping.source_aspect {
            if !aspect.is_finite() || aspect <= 0.0 {
                return Err(ConfigError::Invalid(
                    "mapping.source_aspect must be a positive number".to_owned(),
                ));
            }
        }
        if let Some(aspect) = self.mapping.target_aspect {
            if !aspect.is_finite() || aspect <= 0.0 {
                return Err(ConfigError::Invalid(
                    "mapping.target_aspect must be a positive number".to_owned(),
                ));
            }
        }
        Ok(self)
    }

    /// Decode the configured token, if any.
    pub fn token_bytes(&self) -> Result<Option<[u8; 16]>, ConfigError> {
        self.token.as_deref().map(decode_hex16).transpose()
    }

    /// Serialize the configuration as TOML.
    pub fn to_toml(&self) -> Result<String, ConfigError> {
        toml::to_string_pretty(self).map_err(|error| ConfigError::Serialize(error.to_string()))
    }

    /// Write the configuration to `path`, creating parent directories.
    pub fn save(&self, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            if !parent.as_os_str().is_empty() {
                std::fs::create_dir_all(parent).map_err(|source| ConfigError::Write {
                    path: path.to_path_buf(),
                    source,
                })?;
            }
        }
        let text = self.to_toml()?;
        std::fs::write(path, text).map_err(|source| ConfigError::Write {
            path: path.to_path_buf(),
            source,
        })
    }
}

/// Return the default configuration file path.
#[must_use]
pub fn default_path() -> Option<PathBuf> {
    if let Some(dir) = std::env::var_os("XDG_CONFIG_HOME") {
        return Some(PathBuf::from(dir).join("absolutetouch").join("config.toml"));
    }
    let home = std::env::var_os("HOME")?;
    Some(
        PathBuf::from(home)
            .join(".config")
            .join("absolutetouch")
            .join("config.toml"),
    )
}

/// Generate a random pairing token.
#[must_use]
pub fn generate_token() -> [u8; 16] {
    let mut token = [0_u8; 16];
    if getrandom::fill(&mut token).is_err() {
        // Entropy is unavailable; fall back to a time-derived token so the CLI
        // can still print something. This path is not cryptographically sound
        // and the wire protocol does not rely on it for confidentiality.
        let now = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |elapsed| elapsed.as_nanos());
        token[..16].copy_from_slice(&now.to_le_bytes());
    }
    token
}

/// Encode a 16-byte token as 32 lowercase hexadecimal characters.
#[must_use]
pub fn encode_hex16(token: &[u8; 16]) -> String {
    use std::fmt::Write as _;

    let mut text = String::with_capacity(32);
    for byte in token {
        let _ = write!(text, "{byte:02x}");
    }
    text
}

/// Decode exactly 32 hexadecimal characters into 16 bytes.
pub fn decode_hex16(text: &str) -> Result<[u8; 16], ConfigError> {
    let text = text.trim();
    if text.len() != 32 {
        return Err(ConfigError::Invalid(format!(
            "token must be 32 hexadecimal characters, got {}",
            text.len()
        )));
    }

    let mut token = [0_u8; 16];
    for (index, pair) in text.as_bytes().chunks_exact(2).enumerate() {
        token[index] = (hex_digit(pair[0]).ok_or_else(|| bad_hex(text))? << 4)
            | hex_digit(pair[1]).ok_or_else(|| bad_hex(text))?;
    }
    Ok(token)
}

fn bad_hex(text: &str) -> ConfigError {
    ConfigError::Invalid(format!("token is not valid hexadecimal: {text}"))
}

const fn hex_digit(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

fn present(value: Option<String>) -> Option<String> {
    value.filter(|text| !text.is_empty())
}

fn parse<T: std::str::FromStr>(value: &str, name: &'static str) -> Result<T, ConfigError> {
    value.parse().map_err(|_| ConfigError::InvalidValue {
        name,
        value: value.to_owned(),
    })
}

#[cfg(test)]
mod tests {
    use super::{Config, Overrides, TransportMode, decode_hex16, encode_hex16, generate_token};
    use crate::inject::DeviceMode;
    use crate::mapping::{AspectMode, Rect};

    #[test]
    fn default_ports_are_stable() {
        let config = Config::default();

        assert_eq!(config.transport, TransportMode::Wifi);
        assert_eq!(config.udp_port, 37_689);
        assert_eq!(config.tcp_port, 37_690);
        assert_eq!(config.bind, "0.0.0.0");
        assert!(config.token.is_none());
        assert_eq!(config.watchdog_ms, 750);
        assert!(config.session_timeout_ms > config.watchdog_ms);
        assert_eq!(config.device, DeviceMode::Pointer);
        assert_eq!(config.mapping.aspect, AspectMode::Preserve);
    }

    #[test]
    fn token_hex_roundtrips() {
        let token = generate_token();
        let text = encode_hex16(&token);

        assert_eq!(text.len(), 32);
        assert_eq!(decode_hex16(&text).expect("decode"), token);
        assert_eq!(
            decode_hex16(&text.to_uppercase()).expect("decode uppercase"),
            token
        );
    }

    #[test]
    fn token_rejects_wrong_length_and_bad_digits() {
        assert!(decode_hex16("00").is_err());
        assert!(decode_hex16(&"z".repeat(32)).is_err());
    }

    #[test]
    fn generated_token_is_not_all_zero() {
        assert_ne!(generate_token(), [0_u8; 16]);
    }

    #[test]
    fn environment_overrides_file_values() {
        let config = Config::default()
            .apply_env_with(|name| match name {
                "ABSOLUTETOUCH_UDP_PORT" => Some("40000".to_owned()),
                "ABSOLUTETOUCH_BACKEND" => Some("mock".to_owned()),
                "ABSOLUTETOUCH_DEVICE" => Some("tablet".to_owned()),
                "ABSOLUTETOUCH_DRY_RUN" => Some("true".to_owned()),
                _ => None,
            })
            .expect("apply env");

        assert_eq!(config.udp_port, 40_000);
        assert_eq!(config.backend, "mock");
        assert_eq!(config.device, DeviceMode::Tablet);
        assert!(config.dry_run);
        assert_eq!(config.tcp_port, 37_690);
    }

    #[test]
    fn invalid_device_name_is_reported() {
        let error = Config::default()
            .apply_env_with(|name| (name == "ABSOLUTETOUCH_DEVICE").then(|| "pen".to_owned()))
            .expect_err("must reject");

        assert!(error.to_string().contains("ABSOLUTETOUCH_DEVICE"));
    }

    #[test]
    fn invalid_environment_value_is_reported() {
        let error = Config::default()
            .apply_env_with(|name| {
                (name == "ABSOLUTETOUCH_UDP_PORT").then(|| "not-a-port".to_owned())
            })
            .expect_err("must reject");

        assert!(error.to_string().contains("ABSOLUTETOUCH_UDP_PORT"));
    }

    #[test]
    fn cli_overrides_win() {
        let overrides = Overrides {
            transport: Some(TransportMode::Usb),
            backend: Some("mock".to_owned()),
            device: Some(DeviceMode::Tablet),
            dry_run: Some(true),
        };
        let config = Config::default().apply_overrides(&overrides);

        assert_eq!(config.transport, TransportMode::Usb);
        assert_eq!(config.backend, "mock");
        assert_eq!(config.device, DeviceMode::Tablet);
        assert!(config.dry_run);
    }

    #[test]
    fn validation_rejects_inconsistent_timeouts() {
        let config = Config {
            watchdog_ms: 1_000,
            session_timeout_ms: 1_000,
            ..Config::default()
        };

        assert!(config.validate().is_err());
    }

    #[test]
    fn validation_rejects_bad_bind_and_target() {
        let bad_bind = Config {
            bind: "localhost".to_owned(),
            ..Config::default()
        };
        assert!(bad_bind.validate().is_err());

        let bad_target = Config {
            mapping: super::MappingConfig {
                target: Rect {
                    x: 60_000,
                    y: 0,
                    width: 10_000,
                    height: 100,
                },
                ..super::MappingConfig::default()
            },
            ..Config::default()
        };
        assert!(bad_target.validate().is_err());
    }

    #[test]
    fn file_roundtrip_preserves_values() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("config.toml");
        let config = Config {
            udp_port: 40_001,
            token: Some(encode_hex16(&generate_token())),
            backend: "mock".to_owned(),
            ..Config::default()
        };

        config.save(&path).expect("save");
        let loaded = Config::from_file(&path).expect("load");

        assert_eq!(loaded, config);
    }

    #[test]
    fn unknown_keys_are_rejected() {
        let directory = tempfile::tempdir().expect("tempdir");
        let path = directory.path().join("config.toml");
        std::fs::write(&path, "udp_port = 40000\nnonsense = true\n").expect("write");

        assert!(Config::from_file(&path).is_err());
    }

    #[test]
    fn the_example_config_is_valid() {
        let path =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("absolutetouch.example.toml");

        let config = Config::from_file(&path).expect("example parses");

        assert_eq!(config.udp_port, 37_689);
        assert_eq!(config.backend, "auto");
        assert_eq!(config.device, DeviceMode::Pointer);
        assert!(config.validate().is_ok());
    }
}
