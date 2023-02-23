use flume::{Receiver, RecvError, Sender};
use tokio::task::JoinSet;
use tracing::{debug, trace, warn};

use std::collections::HashMap;

use crate::{
    arp::packet::ArpPacket,
    chassis::{
        LinkLayerId, LinkNetworkPayload, MidLevelProcess, NetworkLayerId, NetworkTransportPayload,
        ProcessMessage, ReceptionResult, TransportLayerId,
    },
    either::ThreeWayEither,
    ethernet::ethertype::EtherType,
    ipv4::addr::IpV4Addr,
    mac::{self, Mac},
};

pub mod packet;

type IpV6Addr = IpV4Addr;

#[derive(Debug)]
pub struct ArpProcess {
    ipv4: Option<(IpV4Addr, HashMap<IpV4Addr, Mac>)>,
    ipv4_handle: (Option<Receiver<IpV4Addr>>, Option<Sender<Mac>>),
    ipv6: Option<(IpV6Addr, HashMap<IpV6Addr, Mac>)>,
}

#[derive(Debug)]
pub struct ArpHandle<Addr, HAddr> {
    rx: Receiver<HAddr>,
    tx: Sender<Addr>,
}

fn get_handle_pair<Addr, HAddr>() -> (ArpHandle<HAddr, Addr>, ArpHandle<Addr, HAddr>) {
    let (tx1, rx1) = flume::unbounded();
    let (tx2, rx2) = flume::unbounded();
    (
        ArpHandle { tx: tx1, rx: rx2 },
        ArpHandle { tx: tx2, rx: rx1 },
    )
}

impl ArpProcess {
    pub fn new(ipv4: Option<IpV4Addr>, ipv6: Option<IpV4Addr>) -> Self {
        Self {
            ipv4: ipv4.map(|ip| (ip, HashMap::new())),
            ipv4_handle: (None, None),
            ipv6: ipv6.map(|ip| (ip, HashMap::new())),
        }
    }

    pub fn new_ipv4_handle(&mut self) -> ArpHandle<IpV4Addr, Mac> {
        let (inner, ext) = get_handle_pair();
        self.ipv4_handle = (Some(inner.rx), Some(inner.tx));
        ext
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
    > for ArpProcess
{
    async fn on_down_message(
        &mut self,
        (source_mac, msg): (Mac, Vec<u8>),
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
        trace!(ARP = ?self, "Recieved from {down_id} {source_mac}: {msg:?}");
        if let Some(arp_packet) = ArpPacket::from_vec(&msg) {
            match (arp_packet.htype, arp_packet.ptype) {
                (1, EtherType::IP_V4) => {
                    if let Some((ip, _)) = self.ipv4 {
                        if ip.as_slice() == arp_packet.target_protocol_address.as_slice() {
                            trace!(ARP = ?self, "Received ARP IPv4 packet: {arp_packet:?}")
                        }
                    }
                }
                (1, EtherType::IP_V6) => {
                    if let Some((ip, _)) = self.ipv6 {
                        if ip.as_slice() == arp_packet.target_protocol_address.as_slice() {
                            trace!(ARP = ?self, "Received ARP IPv6 packet: {arp_packet:?}")
                        }
                    }
                }
                (1, x) => warn!(ARP = ?self, "Unknown ptype: {x:?}"),
                (x, _) => warn!(ARP = ?self, "Unknown htype: {x}"),
            }
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
        warn!(ARP = ?self, "Recieved packet from {up_id:?}");
        // for (id, sender) in down_sender {
        //     trace!(IP = ?self.ip, "Sending IPv4 packet to interface: {id}");
        //     sender
        //         .send_async(ProcessMessage::Message(
        //             NetworkLayerId::Ipv4,
        //             (
        //                 BROADCAST,
        //                 Ipv4Packet::new(ipv4::addr::BROADCAST, self.ip, vec![0x69, 0x69]).to_vec(),
        //             ),
        //         ))
        //         .await
        //         .unwrap();
        // }
    }
    type Extra = ReceptionResult<IpV4Addr>;

    async fn setup(
        &mut self,
        join_set: &mut JoinSet<
            ThreeWayEither<
                ReceptionResult<ProcessMessage<LinkLayerId, NetworkLayerId, LinkNetworkPayload>>,
                ReceptionResult<
                    ProcessMessage<TransportLayerId, NetworkLayerId, NetworkTransportPayload>,
                >,
                Self::Extra,
            >,
        >,
    ) {
        if let Some(rx) = self.ipv4_handle.0.take() {
            join_set.spawn(async move { ThreeWayEither::C((rx.recv_async().await, rx)) });
        }
    }
    async fn on_extra_message(
        &mut self,
        (msg, rx): Self::Extra,
        down_sender: &HashMap<
            LinkLayerId,
            Sender<ProcessMessage<NetworkLayerId, LinkLayerId, LinkNetworkPayload>>,
        >,
        _: &HashMap<
            TransportLayerId,
            Sender<ProcessMessage<NetworkLayerId, TransportLayerId, NetworkTransportPayload>>,
        >,
        join_set: &mut JoinSet<
            ThreeWayEither<
                ReceptionResult<ProcessMessage<LinkLayerId, NetworkLayerId, LinkNetworkPayload>>,
                ReceptionResult<
                    ProcessMessage<TransportLayerId, NetworkLayerId, NetworkTransportPayload>,
                >,
                Self::Extra,
            >,
        >,
    ) {
        match msg {
            Ok(ip) => {
                if let Some((_, table)) = self.ipv4.as_mut() {
                    if let Some(addr) = table.get(&ip) {
                        let _ = self.ipv4_handle.1.as_ref().unwrap().send_async(*addr).await;
                    } else {
                        for (id, sender) in down_sender {
                            match id {
                                LinkLayerId::Ethernet(_, sha) => {
                                    let packet = ArpPacket::new_request(
                                        1,
                                        EtherType::IP_V4,
                                        sha.as_slice().to_vec(),
                                        self.ipv4.as_ref().unwrap().0.as_slice().to_vec(),
                                        ip.as_slice().to_vec(),
                                    );
                                    let _ = sender
                                        .send_async(ProcessMessage::Message(
                                            NetworkLayerId::Arp,
                                            (mac::BROADCAST, packet.to_vec()),
                                        ))
                                        .await;
                                }
                            }
                        }
                    }
                }
            }
            Err(e) => warn!(ARP = ?self, "Error receiving extra message: {e:?}"),
        }
        join_set.spawn(async move { ThreeWayEither::C((rx.recv_async().await, rx)) });
    }
}
