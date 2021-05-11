use clap::{AppSettings, Clap};

#[derive(Clap)]
#[clap(setting = AppSettings::ColoredHelp)]
pub struct Opts {
    #[clap(subcommand)]
    pub subcmd: SubCommand,
}

#[derive(Clap)]
pub enum SubCommand {
    Run(Run),
    Schema,
}

#[derive(Clap)]
pub struct Run {
    #[clap(short, long)]
    pub cert_path: String,
    #[clap(short, long)]
    pub key_path: String,
    #[clap(long, default_value = "127.0.0.1:8443")]
    pub listen_addr: String,
    #[clap(long, default_value = "127.0.0.1")]
    pub rtc_ip: String,
    #[clap(long)]
    pub rtc_announce_ip: Option<String>,
}
