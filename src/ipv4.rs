use std::collections::HashMap;

use flume::Sender;
use tracing::{debug, trace};

use crate::{
    chassis::{
        LinkLayerId, LinkNetworkPayload, MidLevelProcess, NetworkLayerId, NetworkTransportPayload,
        ProcessMessage, TransportLayerId,
    },
    ipv4::packet::Ipv4Packet,
    mac,
};

use self::config::IpV4Config;

pub mod addr;
pub mod config;
pub mod packet;
pub mod protocol;

struct IpV4Process {
    config: IpV4Config,
}

impl IpV4Process {
    fn new(config: IpV4Config) -> Self {
        Self { config }
    }
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
        let ip = self.config.read().await.addr;
        trace!(IP = ?ip, "Recieved from {down_id} {source_mac}: {msg:?}");
        if let Some(ip_packet) = Ipv4Packet::from_vec(&msg) {
            debug!(IP = ?ip, "Recieved IP packet: {ip_packet:?}")
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
        let ip = self.config.read().await.addr;
        trace!(IP = ?ip, "Recieved packet from {up_id:?}");
        for (id, sender) in down_sender {
            trace!(IP = ?ip, "Sending IPv4 packet to interface: {id}");
            sender
                .send_async(ProcessMessage::Message(
                    NetworkLayerId::Ipv4,
                    (
                        mac::BROADCAST,
                        Ipv4Packet::new(addr::BROADCAST, ip, vec![0x69, 0x69]).to_vec(),
                    ),
                ))
                .await
                .unwrap();
        }
    }
}
