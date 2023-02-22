use std::collections::HashMap;

use chassis::{MidLevelProcess, NetworkLayerId, ProcessMessage, TransportLayerId};
use either::Either;
use flume::Sender;
use futures::FutureExt;
use tokio::task::JoinSet;
use tracing::{debug, info, trace, warn};

use crate::{
    chassis::{Chassis, LinkLayerId},
    ethernet::{nic::Nic, packet::EthernetPacket},
    ipv4::{addr::IpV4Addr, packet::Ipv4Packet},
    mac::{Mac, BROADCAST},
};

mod chassis;
mod duplex_conn;
mod ethernet;
mod ipv4;
mod mac;
mod route;
// TODO ARP

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!("Started process");
    let mut authority = mac::authority::SequentialAuthority::new([0, 0x69, 0x69]);
    let mut nic_a = Nic::new(&mut authority);
    let mut nic_b = Nic::new(&mut authority);
    nic_b.connect(&mut nic_a);
    let mut chassis_a = Chassis::new();
    chassis_a.add_nic_with_id(LinkLayerId::Ethernet(0), nic_a);
    let mut chassis_b = Chassis::new();
    chassis_b.add_nic_with_id(LinkLayerId::Ethernet(0), nic_b);
    chassis_b.add_network_layer_process(
        chassis::NetworkLayerId::Ipv4,
        IpV4Process {
            ip: IpV4Addr::new([192, 168, 0, 30]),
        },
    );
    let tx = chassis_a.add_network_layer_process(
        chassis::NetworkLayerId::Ipv4,
        IpV4Process {
            ip: IpV4Addr::new([192, 168, 0, 31]),
        },
    );
    tx.send(ProcessMessage::Message(TransportLayerId::Tcp, ()))
        .unwrap();
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    info!("Stopped process");
}

struct IpV4Process {
    ip: IpV4Addr,
}

#[async_trait::async_trait]
impl MidLevelProcess<NetworkLayerId, TransportLayerId, LinkLayerId, (Mac, Vec<u8>), ()>
    for IpV4Process
{
    async fn on_down_message(
        &mut self,
        (source_mac, msg): (Mac, Vec<u8>),
        down_id: LinkLayerId,
        down_sender: &HashMap<
            LinkLayerId,
            Sender<ProcessMessage<NetworkLayerId, LinkLayerId, (Mac, Vec<u8>)>>,
        >,
        up_sender: &HashMap<
            TransportLayerId,
            Sender<ProcessMessage<NetworkLayerId, TransportLayerId, ()>>,
        >,
    ) {
        trace!(IP = ?self.ip, "Recieved from {down_id} {source_mac}: {msg:?}");
        if let Some(ip_packet) = ipv4::packet::Ipv4Packet::from_vec(&msg) {
            debug!(IP = ?self.ip, "Recieved IP packet: {ip_packet:?}")
        }
    }
    async fn on_up_message(
        &mut self,
        msg: (),
        up_id: TransportLayerId,
        down_sender: &HashMap<
            LinkLayerId,
            Sender<ProcessMessage<NetworkLayerId, LinkLayerId, (Mac, Vec<u8>)>>,
        >,
        up_sender: &HashMap<
            TransportLayerId,
            Sender<ProcessMessage<NetworkLayerId, TransportLayerId, ()>>,
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
