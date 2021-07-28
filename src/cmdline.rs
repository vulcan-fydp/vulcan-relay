use std::str::FromStr;

use crate::built_info;
use clap::{AppSettings, Clap};

#[derive(Clap)]
#[clap(version = built_info::PKG_VERSION, author = built_info::PKG_AUTHORS)]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct Opts {
    /// Path to certificate to use for control and signal endpoints.
    #[clap(short, long, required_unless_present("no-tls"))]
    pub cert_path: Option<String>,
    /// Path to certificate key to use for control and signal endpoints.
    #[clap(short, long, required_unless_present("no-tls"))]
    pub key_path: Option<String>,
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
    /// Disable TLS for all endpoints.
    #[clap(long, conflicts_with_all(&["cert-path", "key-path"]))]
    pub no_tls: bool,
    /// Enable log tags for mediasoup.
    #[clap(short, long, possible_values(&["info", "ice", "dtls", "rtp", "srtp",
        "rtcp", "rtx", "bwe", "score", "simulcast", "svc", "sctp", "message"]))]
    pub log_tags: Vec<WorkerLogTag>,
}

pub struct WorkerLogTag(pub mediasoup::worker::WorkerLogTag);

impl FromStr for WorkerLogTag {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        use mediasoup::worker::WorkerLogTag;
        match s {
            "info" => Ok(Self(WorkerLogTag::Info)),
            "ice" => Ok(Self(WorkerLogTag::Ice)),
            "dtls" => Ok(Self(WorkerLogTag::Dtls)),
            "rtp" => Ok(Self(WorkerLogTag::Rtp)),
            "srtp" => Ok(Self(WorkerLogTag::Srtp)),
            "rtcp" => Ok(Self(WorkerLogTag::Rtcp)),
            "rtx" => Ok(Self(WorkerLogTag::Rtx)),
            "bwe" => Ok(Self(WorkerLogTag::Bwe)),
            "score" => Ok(Self(WorkerLogTag::Score)),
            "simulcast" => Ok(Self(WorkerLogTag::Simulcast)),
            "svc" => Ok(Self(WorkerLogTag::Svc)),
            "sctp" => Ok(Self(WorkerLogTag::Sctp)),
            "message" => Ok(Self(WorkerLogTag::Message)),
            _ => Err(s.to_owned()),
        }
    }
}
