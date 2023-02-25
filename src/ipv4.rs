use std::collections::HashMap;

use flume::Sender;
use tracing::{debug, trace, warn};

use crate::{
    arp::ArpHandle,
    chassis::{
        LinkLayerId, LinkNetworkPayload, MidLevelProcess, NetworkLayerId, NetworkTransportMessage,
        NetworkTransportPayload, ProcessMessage, TransportLayerId,
    },
    ipv4::{packet::{IpV4Header, Ipv4Packet}, self},
    mac::{self, Mac},
};

use self::{addr::IpV4Addr, config::IpV4Config};

pub mod addr;
pub mod config;
pub mod packet;
pub mod protocol;

pub struct IpV4Process {
    config: IpV4Config,
    arp: ArpHandle<IpV4Addr, Mac>,
}

impl IpV4Process {
    pub fn new(config: IpV4Config, arp: ArpHandle<IpV4Addr, Mac>) -> Self {
        Self { config, arp }
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
        if let Some(mut ip_packet) = Ipv4Packet::from_vec(&msg) {
            trace!(IP = ?ip, "Recieved IP packet: {ip_packet:?}");
            if ip_packet.header.destination == ip {
                // TODO fragmented packets`
                if let Some(up_id) = match ip_packet.header.protocol {
                    protocol::ProtocolType::TCP => Some(TransportLayerId::Tcp),
                    protocol::ProtocolType::UDP => Some(TransportLayerId::Udp),
                    protocol::ProtocolType::ICMP => Some(TransportLayerId::Icmp),
                    x => {warn!(IP = ?ip, "Unknown IP protocol: {x:?}"); None}
                } {
                    
                    if let Some(sender) = up_sender.get(&up_id) {
                        let _ = sender.send_async(ProcessMessage::Message(NetworkLayerId::Ipv4, NetworkTransportMessage::IPv4(ip_packet.header.source, ip_packet.payload))).await;
                    }
                }
            } else if ip_packet.header.time_to_live > 0 {
                ip_packet.header.time_to_live -= 1;
                if let Some((next_hop, iface)) = self
                    .config
                    .read()
                    .await
                    .routing
                    .get_route(ip_packet.header.destination)
                {
                    if let Some(Ok(dest_mac)) = self
                        .arp
                        .get_haddr_timeout(next_hop, std::time::Duration::from_secs(1))
                        .await
                    {
                        trace!(IP = ?ip, "Sending IPv4 packet to interface: {iface} next_hop {next_hop} ({dest_mac})");
                        if let Some(sender) = down_sender.get(&iface) {
                            let _ = sender
                                .send_async(ProcessMessage::Message(
                                    NetworkLayerId::Ipv4,
                                    (dest_mac, ip_packet.to_vec()),
                                ))
                                .await;
                        }
                    }
                } else {
                    warn!(IP = ?ip, "Can't find route to {}", ip_packet.header.destination);
                }
            } else {
                warn!(IP = ?ip, "Dropped packet");
            }
        } else {
            warn!(IP = ?ip, "Unable to decode IP packet")
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
        _: &HashMap<
            TransportLayerId,
            Sender<ProcessMessage<NetworkLayerId, TransportLayerId, NetworkTransportPayload>>,
        >,
    ) {
        #[allow(irrefutable_let_patterns)]
        if let NetworkTransportMessage::IPv4(target_ip, msg) = msg {
            let ptype = match up_id {
                TransportLayerId::Tcp => protocol::ProtocolType::TCP,
                TransportLayerId::Udp => protocol::ProtocolType::UDP,
                TransportLayerId::Icmp => protocol::ProtocolType::ICMP,
            };
            let ip = self.config.read().await.addr;
            trace!(IP = ?ip, msg = ?msg, "Recieved packet from {up_id:?} towards {target_ip}");
            if let Some((next_hop, iface)) = self.config.read().await.routing.get_route(target_ip) {
                if let Some(Ok(dest_mac)) = self
                    .arp
                    .get_haddr_timeout(next_hop, std::time::Duration::from_secs(1))
                    .await
                {
                    trace!(IP = ?ip, "Sending IPv4 packet to interface: {iface} next_hop {next_hop} ({dest_mac})");
                    if let Some(sender) = down_sender.get(&iface) {
                        sender
                            .send_async(ProcessMessage::Message(
                                NetworkLayerId::Ipv4,
                                (
                                    dest_mac,
                                    Ipv4Packet::new(
                                        IpV4Header::new(
                                            0,
                                            packet::Ecn::NotECT,
                                            msg.len() as u16,
                                            0,
                                            packet::Flags::empty(),
                                            0,
                                            255,
                                            ptype,
                                            next_hop,
                                            ip,
                                            vec![],
                                        ),
                                        msg,
                                    )
                                    .to_vec(),
                                ),
                            ))
                            .await
                            .unwrap();
                    }
                }
            } else {
                warn!(IP = ?ip, "Can't find route to {target_ip}");
            }
        }
    }
}
