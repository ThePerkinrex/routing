use routing::{network::ipv4::addr::IpV4Addr, transport::icmp::IcmpApi};

use crate::ctrlc::CtrlC;

#[derive(Debug, clap::Args)]
pub struct Traceroute {
    ip: IpV4Addr,
    #[arg(long, short, default_value_t = 5.)]
    timeout_secs: f32,
    #[arg(short)]
    max_hops: Option<usize>,
}

pub async fn traceroute(
    Traceroute {
        ip,
        timeout_secs,
        max_hops,
    }: Traceroute,
    icmp_api: &IcmpApi,
    ctrlc: &CtrlC,
) {
    // TODO ICMP API and UDP
}
