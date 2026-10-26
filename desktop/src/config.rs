//! Configuration types and defaults.

/// Transport selected for a session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum TransportMode {
    /// Wi-Fi UDP.
    #[default]
    Wifi,
    /// USB via usbmuxd.
    Usb,
}

/// Top-level configuration.
///
/// M1 adds file and environment loading. The defaults here define the stable
/// ports and bind address used by the CLI.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Config {
    /// Transport to use.
    pub transport: TransportMode,
    /// UDP port used by the Wi-Fi transport.
    pub udp_port: u16,
    /// TCP port used by the USB transport.
    pub tcp_port: u16,
    /// Address the UDP listener binds to.
    pub bind: String,
    /// Optional pairing token.
    pub token: Option<String>,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            transport: TransportMode::Wifi,
            udp_port: 37_689,
            tcp_port: 37_690,
            bind: "0.0.0.0".to_owned(),
            token: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{Config, TransportMode};

    #[test]
    fn default_ports_are_stable() {
        let config = Config::default();

        assert_eq!(config.transport, TransportMode::Wifi);
        assert_eq!(config.udp_port, 37_689);
        assert_eq!(config.tcp_port, 37_690);
        assert_eq!(config.bind, "0.0.0.0");
        assert!(config.token.is_none());
    }
}
