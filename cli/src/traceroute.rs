use std::sync::Arc;

use flume::RecvError;
use routing::{
    network::ipv4::addr::IpV4Addr,
    transport::{icmp::IcmpApi, udp::UdpHandleGeneric},
};
use tracing::{info, trace, warn};

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
        mut max_hops,
    }: Traceroute,
    icmp_api: &IcmpApi,
    _ctrlc: &CtrlC,
    (udp_handle, ..): &(UdpHandleGeneric<IpV4Addr>,),
) {
    let socket = udp_handle.get_socket(50000).await.unwrap(); // TODO Get random port
    trace!("Aquired socket");
    let handler = Arc::new(icmp_api.get_ttl_handler().await.unwrap());
    trace!("Aquired handler");
    let mut i = 0;
    while max_hops.map(|x| x > 0).unwrap_or(true) {
        max_hops = max_hops.map(|x| x - 1);
        socket.send_ttl((ip, 0), vec![0x69, 0x69], i).await;
        match tokio::time::timeout(
            std::time::Duration::from_secs_f32(timeout_secs),
            handler.recv_async(),
        )
        .await
        {
            Ok(x) => match x {
                Ok((addr, _payload)) => {
                    info!("[HOP {i}] {addr}");
                    if addr == ip {
                        break;
                    }
                }
                Err(RecvError::Disconnected) => {
                    warn!("Handler disconnected");
                    break;
                }
            },
            Err(_) => {
                warn!("Timeout");
                break;
            }
        }
        i += 1;
    }
    if max_hops == Some(0) {
        warn!("Max hops reached");
    }
}
