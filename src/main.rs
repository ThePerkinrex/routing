use std::collections::HashMap;

use chassis::{
    LinkNetworkPayload, MidLevelProcess, NetworkLayerId, NetworkTransportPayload, ProcessMessage,
    TransportLayerId,
};
use flume::Sender;
use tracing::{debug, info, trace, warn};

use crate::{
    arp::ArpProcess,
    chassis::{Chassis, LinkLayerId},
    ethernet::{nic::Nic, packet::EthernetPacket},
    ipv4::{
        addr::{IpV4Addr, IpV4Mask},
        config::IpV4Config,
        packet::Ipv4Packet,
        IpV4Process,
    },
    mac::{Mac, BROADCAST},
    route::RoutingEntry,
};

mod arp;
mod chassis;
mod duplex_conn;
mod either;
mod ethernet;
mod ipv4;
mod mac;
mod route;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!("Started process");
    let mut authority = mac::authority::SequentialAuthority::new([0, 0x69, 0x69]);
    let mut nic_a = Nic::new(&mut authority);
    let mut nic_b = Nic::new(&mut authority);
    nic_b.connect(&mut nic_a);
    let mut chassis_a = Chassis::new();
    chassis_a.add_nic_with_id(0, nic_a);
    let mut chassis_b = Chassis::new();
    chassis_b.add_nic_with_id(1, nic_b);
    let ipv4_config = IpV4Config::default();
    ipv4_config.write().await.addr = IpV4Addr::new([192, 168, 0, 30]);
    let mut arp_b = ArpProcess::new(Some(ipv4_config.clone()), None);
    chassis_b.add_network_layer_process(
        chassis::NetworkLayerId::Ipv4,
        IpV4Process::new(ipv4_config, arp_b.new_ipv4_handle()),
    );
    chassis_b.add_network_layer_process(chassis::NetworkLayerId::Arp, arp_b);
    let ipv4_config = IpV4Config::default();
    ipv4_config.write().await.addr = IpV4Addr::new([192, 168, 0, 31]);
    ipv4_config
        .write()
        .await
        .routing
        .add_route(RoutingEntry::new(
            ipv4::addr::DEFAULT,
            IpV4Addr::new([192, 168, 0, 30]),
            IpV4Mask::new(0),
            LinkLayerId::Ethernet(0, mac::BROADCAST),
        ));
    let mut arp_p = ArpProcess::new(Some(ipv4_config.clone()), None);
    let handle = arp_p.new_ipv4_handle();
    let tx = chassis_a.add_network_layer_process(
        chassis::NetworkLayerId::Ipv4,
        IpV4Process::new(ipv4_config, handle),
    );
    chassis_a.add_network_layer_process(chassis::NetworkLayerId::Arp, arp_p);
    tx.send(ProcessMessage::Message(
        TransportLayerId::Tcp,
        chassis::NetworkTransportMessage::IPv4(IpV4Addr::new([192, 168, 0, 30]), vec![0x69]),
    ))
    .unwrap();
    // debug!(
    //     "Haddr: {:#?}",
    //     handle
    //         .get_haddr_timeout(
    //             IpV4Addr::new([192, 168, 0, 30]),
    //             std::time::Duration::from_millis(100)
    //         )
    //         .await
    // );
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    info!("Stopped process");
}
