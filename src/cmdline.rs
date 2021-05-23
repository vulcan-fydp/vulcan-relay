use crate::built_info;
use clap::{AppSettings, Clap};

#[derive(Clap)]
#[clap(version = built_info::PKG_VERSION, author = built_info::PKG_AUTHORS)]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct Opts {
    #[clap(short, long)]
    pub cert_path: String,
    #[clap(short, long)]
    pub key_path: String,
    #[clap(long, default_value = "127.0.0.1:8443")]
    pub signal_addr: String,
    #[clap(long, default_value = "127.0.0.1:9443")]
    pub control_addr: String,
    #[clap(long, default_value = "127.0.0.1")]
    pub rtc_ip: String,
    #[clap(long)]
    pub rtc_announce_ip: Option<String>,
}
