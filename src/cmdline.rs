use crate::built_info;
use clap::{AppSettings, Clap};

#[derive(Clap)]
#[clap(version = built_info::PKG_VERSION, author = built_info::PKG_AUTHORS)]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct Opts {
    /// Path to certificate to use for control and signal endpoints.
    #[clap(short, long)]
    pub cert_path: String,
    /// Path to certificate key to use for control and signal endpoints.
    #[clap(short, long)]
    pub key_path: String,
    /// Listen address for signal endpoint.
    #[clap(long, default_value = "127.0.0.1:8443")]
    pub signal_addr: String,
    /// Listen address for control endpoint.
    #[clap(long, default_value = "127.0.0.1:9443")]
    pub control_addr: String,
    /// Listen address for RTC protocols.
    #[clap(long, default_value = "127.0.0.1")]
    pub rtc_ip: String,
    /// Announce address for RTC protocols.
    #[clap(long)]
    pub rtc_announce_ip: Option<String>,
    /// Disable TLS for control endpoint only.
    #[clap(long)]
    pub control_no_tls: bool,
}
