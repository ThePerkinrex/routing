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
    ipv4::{addr::IpV4Addr, packet::Ipv4Packet},
    mac::{Mac, BROADCAST},
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
    chassis_b.add_network_layer_process(
        chassis::NetworkLayerId::Ipv4,
        IpV4Process {
            ip: IpV4Addr::new([192, 168, 0, 30]),
        },
    );
    chassis_b.add_network_layer_process(
        chassis::NetworkLayerId::Arp,
        ArpProcess::new(Some(IpV4Addr::new([192, 168, 0, 30])), None),
    );
    let tx = chassis_a.add_network_layer_process(
        chassis::NetworkLayerId::Ipv4,
        IpV4Process {
            ip: IpV4Addr::new([192, 168, 0, 31]),
        },
    );
    let mut arp_p = ArpProcess::new(Some(IpV4Addr::new([192, 168, 0, 31])), None);
    let handle = arp_p.new_ipv4_handle();
    chassis_a.add_network_layer_process(
        chassis::NetworkLayerId::Arp,
        arp_p,
    );
    
    tx.send(ProcessMessage::Message(
        TransportLayerId::Tcp,
        chassis::NetworkTransportMessage::IPv4(IpV4Addr::new([192, 168, 0, 30]), vec![0x69]),
    ))
    .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    info!("Stopped process");
}

struct IpV4Process {
    ip: IpV4Addr,
}

#[async_trait::async_trait]
impl
    MidLevelProcess<
        NetworkLayerId,
        TransportLayerId,
        LinkLayerId,
        LinkNetworkPayload,
        NetworkTransportPayload,
    > for IpV4Process
{
    type Extra = ();
    async fn on_down_message(
        &mut self,
        (source_mac, msg): LinkNetworkPayload,
        down_id: LinkLayerId,
        down_sender: &HashMap<
            LinkLayerId,
            Sender<ProcessMessage<NetworkLayerId, LinkLayerId, LinkNetworkPayload>>,
        >,
        up_sender: &HashMap<
            TransportLayerId,
            Sender<ProcessMessage<NetworkLayerId, TransportLayerId, NetworkTransportPayload>>,
        >,
    ) {
        trace!(IP = ?self.ip, "Recieved from {down_id} {source_mac}: {msg:?}");
        if let Some(ip_packet) = ipv4::packet::Ipv4Packet::from_vec(&msg) {
            debug!(IP = ?self.ip, "Recieved IP packet: {ip_packet:?}")
        }
    }
    async fn on_up_message(
        &mut self,
        msg: NetworkTransportPayload,
        up_id: TransportLayerId,
        down_sender: &HashMap<
            LinkLayerId,
            Sender<ProcessMessage<NetworkLayerId, LinkLayerId, LinkNetworkPayload>>,
        >,
        up_sender: &HashMap<
            TransportLayerId,
            Sender<ProcessMessage<NetworkLayerId, TransportLayerId, NetworkTransportPayload>>,
        >,
    ) {
        trace!(IP = ?self.ip, "Recieved packet from {up_id:?}");
        for (id, sender) in down_sender {
            trace!(IP = ?self.ip, "Sending IPv4 packet to interface: {id}");
            sender
                .send_async(ProcessMessage::Message(
                    NetworkLayerId::Ipv4,
                    (
                        BROADCAST,
                        Ipv4Packet::new(ipv4::addr::BROADCAST, self.ip, vec![0x69, 0x69]).to_vec(),
                    ),
                ))
                .await
                .unwrap();
        }
    }
}
