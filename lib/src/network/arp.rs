use chrono::{DateTime, Local};
use either::Either;
use flume::{Receiver, RecvError, Sender};
use tokio::task::JoinSet;
use tracing::{trace, warn};

use std::{collections::HashMap, sync::Arc, time::Duration};

use super::ipv4::{addr::IpV4Addr, config::IpV4Config};
use crate::{
    chassis::{
        LinkLayerId, LinkNetworkPayload, MidLevelProcess, NetworkLayerId, NetworkTransportPayload,
        ProcessMessage, ReceptionResult, TransportLayerId,
    },
    either::ThreeWayEither,
    link::ethernet::ethertype::EtherType,
    mac::{self, Mac},
};
use packet::{ArpPacket, Operation};

pub mod packet;

type IpV6Addr = IpV4Addr;

#[derive(Debug)]
pub struct ArpProcess {
    ipv4: Option<(
        IpV4Config,
        HashMap<(IpV4Addr, LinkLayerId), (Mac, DateTime<Local>)>,
    )>,
    ipv4_handle: Option<(Arc<Receiver<(IpV4Addr, LinkLayerId)>>, Sender<Mac>)>,
    ipv6: Option<(
        IpV6Addr,
        HashMap<(IpV6Addr, LinkLayerId), (Mac, DateTime<Local>)>,
    )>,
    get_new_ipv4_handle: (
        Arc<Receiver<()>>,
        Sender<ArpHandle<(IpV4Addr, LinkLayerId), Mac>>,
    ),
    get_ipv4_table: (
        Arc<Receiver<()>>,
        Sender<HashMap<(IpV4Addr, LinkLayerId), (Mac, DateTime<Local>)>>,
    ),
}

impl ArpProcess {
    pub fn new(ipv4: Option<IpV4Config>, ipv6: Option<IpV4Addr>) -> (Self, GenericArpHandle) {
        let (new_ipv4_handle_external_tx, new_ipv4_handle_internal_rx) = flume::unbounded();
        let (new_ipv4_handle_internal_tx, new_ipv4_handle_external_rx) = flume::unbounded();
        let (get_ipv4_table_external_tx, get_ipv4_table_internal_rx) = flume::unbounded();
        let (get_ipv4_table_internal_tx, get_ipv4_table_external_rx) = flume::unbounded();
        (
            Self {
                ipv4: ipv4.map(|ip| (ip, HashMap::new())),
                ipv4_handle: None,
                ipv6: ipv6.map(|ip| (ip, HashMap::new())),
                get_new_ipv4_handle: (
                    Arc::new(new_ipv4_handle_internal_rx),
                    new_ipv4_handle_internal_tx,
                ),
                get_ipv4_table: (
                    Arc::new(get_ipv4_table_internal_rx),
                    get_ipv4_table_internal_tx,
                ),
            },
            GenericArpHandle {
                get_new_ipv4_handle: (new_ipv4_handle_external_tx, new_ipv4_handle_external_rx),
                get_ipv4_table: (get_ipv4_table_external_tx, get_ipv4_table_external_rx),
            },
        )
    }

    pub fn new_ipv4_handle(&mut self) -> ArpHandle<(IpV4Addr, LinkLayerId), Mac> {
        let (inner, ext) = get_handle_pair();
        self.ipv4_handle = Some((Arc::new(inner.rx), inner.tx));
        ext
    }
}

pub enum ExtraMessage {
    GetIpV4(Result<(IpV4Addr, LinkLayerId), RecvError>),
    NewIpV4(Result<(), RecvError>),
    GetCurrentIPv4Table(Result<(), RecvError>),
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
        _: &HashMap<
            TransportLayerId,
            Sender<ProcessMessage<NetworkLayerId, TransportLayerId, NetworkTransportPayload>>,
        >,
    ) {
        trace!(ARP = ?self, "Recieved from {down_id} {source_mac}: {msg:?}");
        if let Some(arp_packet) = ArpPacket::from_vec(&msg) {
            match (arp_packet.htype, arp_packet.ptype, down_id) {
                (1, EtherType::IP_V4, LinkLayerId::Ethernet(_, mac)) => {
                    if let Some((ip, table)) = &mut self.ipv4 {
                        if let (Ok(sha), Ok(spa)) = (
                            arp_packet.sender_harware_address.as_slice().try_into(),
                            arp_packet.sender_protocol_address.as_slice().try_into(),
                        ) {
                            let ip = IpV4Addr::new(spa);
                            let mac = Mac::new(sha);
                            table.entry((ip, down_id)).or_insert((mac, Local::now()));
                            trace!("ARP: Tried to add pair {ip} -> {mac} to the table");
                        }

                        if ip.read().await.addr.as_slice()
                            == arp_packet.target_protocol_address.as_slice()
                        {
                            match arp_packet.operation {
                                Operation::Request => {
                                    // trace!(ARP = ?self, "Received ARP IPv4 Request packet: {arp_packet:?}");
                                    let reply = ArpPacket::new_reply(
                                        arp_packet.htype,
                                        arp_packet.ptype,
                                        mac.as_slice().to_vec(),
                                        ip.read().await.addr.as_slice().to_vec(),
                                        arp_packet.sender_harware_address,
                                        arp_packet.sender_protocol_address,
                                    );
                                    let _ = down_sender[&down_id]
                                        .send_async(ProcessMessage::Message(
                                            NetworkLayerId::Arp,
                                            (source_mac, reply.to_vec()),
                                        ))
                                        .await;
                                }
                                Operation::Reply => {
                                    // trace!(ARP = ?self, "Received ARP IPv4 Reply packet: {arp_packet:?}");

                                    if let Some((_, tx)) = self.ipv4_handle.as_ref() {
                                        let _ = tx
                                            .send_async(Mac::new(
                                                arp_packet
                                                    .sender_harware_address
                                                    .try_into()
                                                    .unwrap(),
                                            ))
                                            .await;
                                    }
                                }
                            }
                        }
                    }
                }
                (1, EtherType::IP_V6, LinkLayerId::Ethernet(_, _)) => {
                    if let Some((ip, _)) = self.ipv6 {
                        if ip.as_slice() == arp_packet.target_protocol_address.as_slice() {
                            trace!(ARP = ?self, "Received ARP IPv6 packet: {arp_packet:?}")
                        }
                    }
                }
                (1, x, _) => warn!(ARP = ?self, "Unknown ptype: {x:?}"),
                (x, _, _) => warn!(ARP = ?self, "Unknown htype: {x}"),
            }
        } else {
            warn!("Couldnt decode ARP packet");
        }
    }
    async fn on_up_message(
        &mut self,
        _: NetworkTransportPayload,
        up_id: TransportLayerId,
        _: &HashMap<
            LinkLayerId,
            Sender<ProcessMessage<NetworkLayerId, LinkLayerId, LinkNetworkPayload>>,
        >,
        _: &HashMap<
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
    type Extra = ExtraMessage;

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
        let new_rx = self.get_new_ipv4_handle.0.clone();
        join_set.spawn(async move {
            ThreeWayEither::C(ExtraMessage::NewIpV4(new_rx.recv_async().await))
        });
        let rx = self.get_ipv4_table.0.clone();
        join_set.spawn(async move {
            ThreeWayEither::C(ExtraMessage::GetCurrentIPv4Table(rx.recv_async().await))
        });
        if let Some((rx, _)) = self.ipv4_handle.as_ref() {
            let rx = rx.clone();
            join_set.spawn(async move {
                ThreeWayEither::C(ExtraMessage::GetIpV4(rx.recv_async().await))
            });
        }
    }
    async fn on_extra_message(
        &mut self,
        msg: Self::Extra,
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
            ExtraMessage::GetCurrentIPv4Table(r) => match r {
                Ok(()) => {
                    if let Some((_, table)) = &self.ipv4 {
                        let _ = self.get_ipv4_table.1.send_async(table.clone()).await;
                    }
                    let rx = self.get_ipv4_table.0.clone();
                    join_set.spawn(async move {
                        ThreeWayEither::C(ExtraMessage::GetCurrentIPv4Table(rx.recv_async().await))
                    });
                }
                Err(RecvError::Disconnected) => warn!("ARP: Disconnected get ipv4 arp table"),
            },
            ExtraMessage::NewIpV4(r) => {
                match r {
                    Ok(()) => {
                        let handle = self.new_ipv4_handle();
                        let _ = self.get_new_ipv4_handle.1.send_async(handle).await;
                        if let Some((rx, _)) = self.ipv4_handle.as_ref() {
                            let rx = rx.clone();
                            join_set.spawn(async move {
                                ThreeWayEither::C(ExtraMessage::GetIpV4(rx.recv_async().await))
                            });
                        }
                    }
                    Err(e) => warn!(ARP = ?self, "Error receiving extra message: {e:?}"),
                }
                let new_rx = self.get_new_ipv4_handle.0.clone();
                join_set.spawn(async move {
                    ThreeWayEither::C(ExtraMessage::NewIpV4(new_rx.recv_async().await))
                });
            }
            ExtraMessage::GetIpV4(ip) => match ip {
                Ok((ip, id)) => {
                    if let Some(ipv4_handle) = &self.ipv4_handle {
                        if let Some((config, table)) = self.ipv4.as_mut() {
                            let ttl = config.read().await.arp_ttl;
                            if let Some(addr) =
                                table.get(&(ip, id)).copied().and_then(|(addr, time)| {
                                    if time - Local::now() > ttl {
                                        table.remove(&(ip, id));
                                        None
                                    } else {
                                        Some(addr)
                                    }
                                })
                            {
                                trace!("ARP: Sending known MAC address ({addr}) for IPv4 {ip}");
                                let _ = ipv4_handle.1.send_async(addr).await;
                            } else {
                                trace!("ARP: Searching for MAC address for IPv4 {ip}");
                                if let Some((id, sender)) = down_sender.get_key_value(&id) {
                                    match id {
                                        LinkLayerId::Ethernet(_, sha) => {
                                            let packet = ArpPacket::new_request(
                                                1,
                                                EtherType::IP_V4,
                                                sha.as_slice().to_vec(),
                                                self.ipv4
                                                    .as_ref()
                                                    .unwrap()
                                                    .0
                                                    .read()
                                                    .await
                                                    .addr
                                                    .as_slice()
                                                    .to_vec(),
                                                ip.as_slice().to_vec(),
                                            );
                                            if let Err(e) = sender
                                                .send_async(ProcessMessage::Message(
                                                    NetworkLayerId::Arp,
                                                    (mac::BROADCAST, packet.to_vec()),
                                                ))
                                                .await
                                            {
                                                warn!("ARP: Error sending ARP request package: {e}")
                                            }
                                        }
                                    }
                                }
                            }
                        }
                        let ip_v4_rx = ipv4_handle.0.clone();
                        join_set.spawn(async move {
                            ThreeWayEither::C(ExtraMessage::GetIpV4(ip_v4_rx.recv_async().await))
                        });
                    } else {
                        warn!("ARP: No IPv4 Handle");
                    }
                }
                Err(RecvError::Disconnected) => {
                    warn!(ARP = ?self, "Disconnected IPv4 handle");
                    self.ipv4_handle = None;
                }
            },
        }
    }
}
#[derive(Debug)]
pub struct ArpHandle<Addr, HAddr> {
    rx: Receiver<HAddr>,
    tx: Sender<Addr>,
}

impl<Addr, HAddr> ArpHandle<Addr, HAddr>
where
    Addr: Send,
    HAddr: Send,
{
    pub async fn get_haddr(
        &self,
        addr: Addr,
    ) -> Result<HAddr, either::Either<flume::SendError<Addr>, RecvError>> {
        self.tx.send_async(addr).await.map_err(Either::Left)?;
        self.rx.recv_async().await.map_err(Either::Right)
    }

    pub async fn get_haddr_timeout(
        &self,
        addr: Addr,
        timeout: Duration,
    ) -> Option<Result<HAddr, either::Either<flume::SendError<Addr>, RecvError>>> {
        tokio::time::timeout(timeout, self.get_haddr(addr))
            .await
            .ok()
    }
}

fn get_handle_pair<Addr, HAddr>() -> (ArpHandle<HAddr, Addr>, ArpHandle<Addr, HAddr>) {
    let (tx1, rx1) = flume::unbounded();
    let (tx2, rx2) = flume::unbounded();
    (
        ArpHandle { tx: tx1, rx: rx2 },
        ArpHandle { tx: tx2, rx: rx1 },
    )
}

pub struct GenericArpHandle {
    get_new_ipv4_handle: (
        Sender<()>,
        Receiver<ArpHandle<(IpV4Addr, LinkLayerId), Mac>>,
    ),

    get_ipv4_table: (
        Sender<()>,
        Receiver<HashMap<(IpV4Addr, LinkLayerId), (Mac, DateTime<Local>)>>,
    ),
}

impl GenericArpHandle {
    pub async fn get_new_ipv4_handle(&self) -> Option<ArpHandle<(IpV4Addr, LinkLayerId), Mac>> {
        self.get_new_ipv4_handle.0.send_async(()).await.ok()?;
        self.get_new_ipv4_handle.1.recv_async().await.ok()
    }

    pub async fn get_ipv4_table(
        &self,
    ) -> Option<HashMap<(IpV4Addr, LinkLayerId), (Mac, DateTime<Local>)>> {
        self.get_ipv4_table.0.send_async(()).await.ok()?;
        self.get_ipv4_table.1.recv_async().await.ok()
    }
}
