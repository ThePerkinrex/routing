use std::{collections::HashMap, fmt::Display, hash::Hash};

use async_trait::async_trait;
use either::Either;
use flume::{Receiver, Sender};
use tokio::task::{JoinHandle, JoinSet};
use tracing::{trace, warn};

use crate::{
    ethernet::{nic::Nic, packet::EthernetPacket},
    mac::Mac,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkLayerId {
    Ethernet(u16),
}

impl Display for LinkLayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ethernet(id) => write!(f, "eth{id}"),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkLayerId {
    Ipv4,
    Ipv6,
    Arp,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum TransportLayerId {
    Tcp,
    Udp,
}

pub enum ProcessMessage<SenderId, ReceiverId, Payload> {
    NewConn(
        SenderId,
        Sender<ProcessMessage<ReceiverId, SenderId, Payload>>,
    ),
    Message(SenderId, Payload),
}

type LinkNetworkPayload = (Mac, Vec<u8>);

type LinkLayerProcessHandle = (
    JoinHandle<()>,
    Sender<ProcessMessage<NetworkLayerId, LinkLayerId, LinkNetworkPayload>>,
);

type MidLayerProcessHandle<DownId, Id, UpId, DownPayload, UpPayload> = (
    JoinHandle<()>,
    Sender<ProcessMessage<DownId, Id, DownPayload>>,
    Sender<ProcessMessage<UpId, Id, UpPayload>>,
);

#[derive(Debug, Default)]
pub struct Chassis {
    link_layer_processes: HashMap<LinkLayerId, LinkLayerProcessHandle>,
    network_layer_processes: HashMap<
        NetworkLayerId,
        MidLayerProcessHandle<
            LinkLayerId,
            NetworkLayerId,
            TransportLayerId,
            LinkNetworkPayload,
            (),
        >,
    >,
}

impl Chassis {
    pub fn new() -> Self {
        Self::default()
    }

    fn add_link_layer_process<
        F: FnOnce(LinkProcessUpLink) -> Fut,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    >(
        &mut self,
        id: LinkLayerId,
        f: F,
    ) {
        self.link_layer_processes.entry(id).or_insert_with(|| {
            let (tx, rx) = flume::unbounded();
            let link = ChassisInProcessLink {
                rx,
                tx: self
                    .network_layer_processes
                    .iter()
                    .map(|(k, (_, v, _))| (*k, v.clone()))
                    .collect::<HashMap<_, _>>(),
            };
            for (_, sender, _) in self.network_layer_processes.values() {
                let _ = sender.send(ProcessMessage::NewConn(id, tx.clone()));
            }
            let handle = tokio::spawn(f(link));
            (handle, tx)
        });
    }

    pub fn add_nic_with_id(&mut self, id: LinkLayerId, nic: Nic) {
        self.add_link_layer_process(id, move |mut up_link| async move {
            let (conn, addr) = nic.split();
            if let Some((tx, rx)) = conn {
                let mut join_set = JoinSet::new();
                // let nic_ref = &nic;
                join_set.spawn(async move { Either::Left((rx.recv_async().await, rx)) });
                join_set.spawn(async move {
                    Either::Right((up_link.rx.recv_async().await, up_link.rx))
                });
                loop {
                    match join_set.join_next().await {
                        Some(Ok(Either::Left((eth_packet, rx)))) => {
                            match eth_packet {
                                Err(_) => warn!(NIC = ?addr, "Error recieving eth packet: Disconnected"),
                                Ok(eth_packet) => {
                                    let dest = eth_packet.get_dest();
                                    if dest == addr || dest.is_multicast() {
                                        trace!(
                                            NIC = ?addr,
                                            packet = ?eth_packet,
                                            "Recieved packet"
                                        );
                                        match eth_packet.get_ether_type() {
                                            0x0800 => {
                                                if let Some(sender) = up_link.tx.get(&NetworkLayerId::Ipv4) {
                                                    let _ = sender.send_async(ProcessMessage::Message(id, (addr, eth_packet.payload))).await.map_err(|e| warn!("Cant send ipv4 packet up: {e:?}"));
                                                }else{
                                                    warn!(NIC = ?addr,"No IPv4 process to send packet")
                                                }
                                            }
                                            x => warn!(NIC = ?addr, "Unknown ether_type {x:x}")
                                        }
                                    }
                                }
                            }
                            join_set
                                .spawn(async move { Either::Left((rx.recv_async().await, rx)) });
                        }
                        Some(Ok(Either::Right((up_link_msg, rx)))) => {
                            match up_link_msg {
                                Ok(up_link_msg) => match up_link_msg {
                                    ProcessMessage::NewConn(upper_id, sender) => {
                                        up_link.tx.insert(upper_id, sender);
                                    }
                                    ProcessMessage::Message(id, (dest, payload)) => match id {
                                        NetworkLayerId::Ipv4 => {
                                            trace!(NIC = ?addr, "Transmitting ipv4 packet");
                                            match EthernetPacket::new_ip_v4(dest, addr, payload) {
                                                Some(packet) => {
                                                    let _ = tx.send_async(packet).await;
                                                }
                                                None => {
                                                    warn!(NIC = ?addr, "Error building ethernet ipv4 packet")
                                                }
                                            }
                                        }
                                        NetworkLayerId::Ipv6 => {
                                            match EthernetPacket::new_ip_v6(dest, addr, payload) {
                                                Some(packet) => {
                                                    let _ = tx.send_async(packet).await;
                                                }
                                                None => {
                                                    warn!(NIC = ?addr, "Error building ethernet ipv6 packet")
                                                }
                                            }
                                        }
                                        NetworkLayerId::Arp => {
                                            match EthernetPacket::new_arp(dest, addr, payload) {
                                                Some(packet) => {
                                                    let _ = tx.send_async(packet).await;
                                                }
                                                None => warn!(NIC = ?addr, "Error building ethernet ARP packet"),
                                            }
                                        }
                                    },
                                },
                                Err(e) => warn!(NIC = ?addr, "Down link packet error: {e:?}"),
                            }
                            join_set
                                .spawn(async move { Either::Right((rx.recv_async().await, rx)) });
                        }
                        Some(Err(e)) => warn!(NIC = ?addr, "join error: {e:?}"),
                        None => (),
                    }
                    tokio::task::yield_now().await
                }
            }
        })
    }

    pub fn add_network_layer_process<
        P: MidLevelProcess<NetworkLayerId, TransportLayerId, LinkLayerId, LinkNetworkPayload, ()>
            + Send
            + 'static,
    >(
        &mut self,
        id: NetworkLayerId,
        process: P,
    ) -> Sender<ProcessMessage<TransportLayerId, NetworkLayerId, ()>> {
        add_mid_level_process(
            id,
            &mut self.network_layer_processes,
            self.link_layer_processes
                .iter()
                .map(|(k, (_, v))| (*k, v.clone()))
                .collect::<HashMap<_, _>>(),
            HashMap::new(),
            build_mid_level_handler(process),
        )
    }
}

fn add_mid_level_process<Id, UpId, DownId, DownPayload, UpPayload, F, Fut>(
    id: Id,
    curr_level: &mut HashMap<Id, MidLayerProcessHandle<DownId, Id, UpId, DownPayload, UpPayload>>,
    down_map: HashMap<DownId, Sender<ProcessMessage<Id, DownId, DownPayload>>>,
    up_map: HashMap<UpId, Sender<ProcessMessage<Id, UpId, UpPayload>>>,
    f: F,
) -> Sender<ProcessMessage<UpId, Id, UpPayload>>
where
    Id: Clone + Eq + Hash,
    F: FnOnce(
        ChassisProcessLink<Id, DownId, DownPayload>,
        ChassisProcessLink<Id, UpId, UpPayload>,
    ) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    let (tx_down, rx_down) = flume::unbounded();
    let (tx_up, rx_up) = flume::unbounded();
    curr_level.entry(id.clone()).or_insert_with(|| {
        for sender in down_map.values() {
            let _ = sender.send(ProcessMessage::NewConn(id.clone(), tx_down.clone()));
        }
        for sender in up_map.values() {
            let _ = sender.send(ProcessMessage::NewConn(id.clone(), tx_up.clone()));
        }
        let downlink = ChassisInProcessLink {
            rx: rx_down,
            tx: down_map,
        };

        let uplink = ChassisInProcessLink {
            rx: rx_up,
            tx: up_map,
        };

        let handle = tokio::spawn(f(downlink, uplink));
        (handle, tx_down, tx_up.clone())
    });
    tx_up
}

fn build_mid_level_handler<
    Id,
    UpId,
    DownId,
    DownPayload,
    UpPayload,
    P: MidLevelProcess<Id, UpId, DownId, DownPayload, UpPayload> + Send + 'static,
>(
    mut process: P,
) -> impl FnOnce(
    ChassisProcessLink<Id, DownId, DownPayload>,
    ChassisProcessLink<Id, UpId, UpPayload>,
) -> std::pin::Pin<Box<dyn std::future::Future<Output = ()> + Send + 'static>>
where
    DownPayload: Send + 'static,
    UpPayload: Send + 'static,
    Id: Send + 'static,
    DownId: Send + Sync + 'static + Eq + Hash,
    UpId: Send + Sync + 'static + Eq + Hash,
{
    move |mut down_link: ChassisProcessLink<Id, DownId, DownPayload>,
          mut up_link: ChassisProcessLink<Id, UpId, UpPayload>| {
        Box::pin(async move {
            let mut join_set = JoinSet::new();
            // let nic_ref = &nic;
            join_set.spawn(
                async move { Either::Left((down_link.rx.recv_async().await, down_link.rx)) },
            );
            join_set
                .spawn(async move { Either::Right((up_link.rx.recv_async().await, up_link.rx)) });
            loop {
                match join_set.join_next().await {
                    Some(Ok(Either::Left((down_packet, rx)))) => {
                        match down_packet {
                            Ok(ProcessMessage::NewConn(down_id, sender)) => {
                                down_link.tx.insert(down_id, sender);
                            }
                            Ok(ProcessMessage::Message(down_id, msg)) => {
                                process
                                    .on_down_message(msg, down_id, &down_link.tx, &up_link.tx)
                                    .await
                            }
                            Err(e) => warn!("Error recieving packet from below: {e:?}"),
                        }
                        join_set.spawn(async move { Either::Left((rx.recv_async().await, rx)) });
                    }
                    Some(Ok(Either::Right((up_link_msg, rx)))) => {
                        match up_link_msg {
                            Ok(up_link_msg) => match up_link_msg {
                                ProcessMessage::NewConn(upper_id, sender) => {
                                    up_link.tx.insert(upper_id, sender);
                                }
                                ProcessMessage::Message(id, msg) => {
                                    process
                                        .on_up_message(msg, id, &down_link.tx, &up_link.tx)
                                        .await
                                }
                            },
                            Err(e) => warn!("Down link packet error: {e:?}"),
                        }
                        join_set.spawn(async move { Either::Right((rx.recv_async().await, rx)) });
                    }
                    Some(Err(e)) => warn!("join error: {e:?}"),
                    None => (),
                }
                tokio::task::yield_now().await
            }
        })
    }
}

#[async_trait]
pub trait MidLevelProcess<Id, UpId, DownId, DownPayload, UpPayload> {
    async fn on_down_message(
        &mut self,
        msg: DownPayload,
        down_id: DownId,
        down_sender: &HashMap<DownId, Sender<ProcessMessage<Id, DownId, DownPayload>>>,
        up_sender: &HashMap<UpId, Sender<ProcessMessage<Id, UpId, UpPayload>>>,
    );
    async fn on_up_message(
        &mut self,
        msg: UpPayload,
        up_id: UpId,
        down_sender: &HashMap<DownId, Sender<ProcessMessage<Id, DownId, DownPayload>>>,
        up_sender: &HashMap<UpId, Sender<ProcessMessage<Id, UpId, UpPayload>>>,
    );
}

#[async_trait]
impl<A, B, Fa, Fb, Id, UpId, DownId, DownPayload, UpPayload>
    MidLevelProcess<Id, UpId, DownId, DownPayload, UpPayload> for (A, B)
where
    for<'a> A: FnMut(
            DownPayload,
            DownId,
            &'a HashMap<DownId, Sender<ProcessMessage<Id, DownId, DownPayload>>>,
            &'a HashMap<UpId, Sender<ProcessMessage<Id, UpId, UpPayload>>>,
        ) -> Fa
        + Send
        + 'static,
    for<'a> B: FnMut(
            UpPayload,
            UpId,
            &'a HashMap<DownId, Sender<ProcessMessage<Id, DownId, DownPayload>>>,
            &'a HashMap<UpId, Sender<ProcessMessage<Id, UpId, UpPayload>>>,
        ) -> Fb
        + Send
        + 'static,
    Fa: std::future::Future<Output = ()> + Send + 'static,
    Fb: std::future::Future<Output = ()> + Send + 'static,
    DownPayload: Send + 'static,
    UpPayload: Send + 'static,
    Id: Send + Sync + 'static,
    DownId: Send + Sync + 'static,
    UpId: Send + Sync + 'static,
{
    async fn on_down_message(
        &mut self,
        msg: DownPayload,
        down_id: DownId,
        down_sender: &HashMap<DownId, Sender<ProcessMessage<Id, DownId, DownPayload>>>,
        up_sender: &HashMap<UpId, Sender<ProcessMessage<Id, UpId, UpPayload>>>,
    ) {
        self.0(msg, down_id, down_sender, up_sender).await
    }
    async fn on_up_message(
        &mut self,
        msg: UpPayload,
        up_id: UpId,
        down_sender: &HashMap<DownId, Sender<ProcessMessage<Id, DownId, DownPayload>>>,
        up_sender: &HashMap<UpId, Sender<ProcessMessage<Id, UpId, UpPayload>>>,
    ) {
        self.1(msg, up_id, down_sender, up_sender).await
    }
}

pub type ChassisProcessLink<Id, LinkedId, Payload> = ChassisInProcessLink<
    LinkedId,
    ProcessMessage<LinkedId, Id, Payload>,
    ProcessMessage<Id, LinkedId, Payload>,
>;

pub type NetworkProcessDownLink = ChassisInProcessLink<
    LinkLayerId,
    ProcessMessage<LinkLayerId, NetworkLayerId, LinkNetworkPayload>,
    ProcessMessage<NetworkLayerId, LinkLayerId, LinkNetworkPayload>,
>;

pub type NetworkProcessUpLink = ChassisInProcessLink<
    TransportLayerId,
    ProcessMessage<TransportLayerId, NetworkLayerId, ()>,
    ProcessMessage<NetworkLayerId, TransportLayerId, ()>,
>;

pub type LinkProcessUpLink = ChassisInProcessLink<
    NetworkLayerId,
    ProcessMessage<NetworkLayerId, LinkLayerId, LinkNetworkPayload>,
    ProcessMessage<LinkLayerId, NetworkLayerId, LinkNetworkPayload>,
>;

pub struct ChassisInProcessLink<LinkedId, RecvMsg, SendMsg> {
    pub rx: Receiver<RecvMsg>,
    pub tx: HashMap<LinkedId, Sender<SendMsg>>,
}
