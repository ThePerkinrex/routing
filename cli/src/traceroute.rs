use routing::{network::ipv4::addr::IpV4Addr, transport::{icmp::IcmpApi, udp::UdpHandleGeneric}};
use tracing::info;

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
    (udp_handle, ..): &(UdpHandleGeneric<IpV4Addr>, )
) {
    let socket = udp_handle.get_socket(50000).await.unwrap(); // TODO Get random port
    info!("Aquired socket");
    socket.send_ttl((ip, 0), vec![0x69, 0x69], 0).await;
    // TODO ICMP API and UDP
}
