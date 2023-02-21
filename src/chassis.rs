use std::{collections::HashMap, hash::Hash};

use barrage::Disconnected;
use either::Either;
use flume::{Receiver, Sender};
use tokio::{
    net,
    task::{JoinHandle, JoinSet},
};
use tracing::{trace, warn};

use crate::{
    ethernet::{nic::Nic, packet::EthernetPacket},
    mac::Mac,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LinkLayerId {
    Ethernet(u16),
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
    NewConn(SenderId, Sender<ProcessMessage<ReceiverId, SenderId, Payload>>),
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
        MidLayerProcessHandle<LinkLayerId, NetworkLayerId, TransportLayerId, LinkNetworkPayload, ()>,
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
                            eth_packet.map_or_else(
                                |_| warn!("Error recieving eth packet: Disconnected"),
                                |eth_packet| {
                                    let dest = eth_packet.get_dest();
                                    if dest == addr || dest.is_multicast() {
                                        trace!(
                                            "NIC {} recieved packet from: {:?}",
                                            addr,
                                            eth_packet
                                        );
                                    }
                                },
                            );
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
                                            trace!("Transmitting ipv4 packet");
                                            match EthernetPacket::new_ip_v4(dest, addr, payload) {
                                                Some(packet) => {
                                                    let _ = tx.send_async(packet).await;
                                                }
                                                None => {
                                                    warn!("Error building ethernet ipv4 packet")
                                                }
                                            }
                                        }
                                        NetworkLayerId::Ipv6 => {
                                            match EthernetPacket::new_ip_v6(dest, addr, payload) {
                                                Some(packet) => {
                                                    let _ = tx.send_async(packet).await;
                                                }
                                                None => {
                                                    warn!("Error building ethernet ipv6 packet")
                                                }
                                            }
                                        }
                                        NetworkLayerId::Arp => {
                                            match EthernetPacket::new_arp(dest, addr, payload) {
                                                Some(packet) => {
                                                    let _ = tx.send_async(packet).await;
                                                }
                                                None => warn!("Error building ethernet ARP packet"),
                                            }
                                        }
                                    },
                                },
                                Err(e) => warn!("Down link packet error: {e:?}"),
                            }
                            join_set
                                .spawn(async move { Either::Right((rx.recv_async().await, rx)) });
                        }
                        Some(Err(e)) => warn!("join error: {e:?}"),
                        None => (),
                    }
                    tokio::task::yield_now().await
                }
            }
        })
    }

    pub fn add_network_layer_process<
        F: FnOnce(NetworkProcessDownLink, NetworkProcessUpLink) -> Fut,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    >(
        &mut self,
        id: NetworkLayerId,
        f: F,
    ) {
        add_mid_level_process(id, &mut self.network_layer_processes, self
                         .link_layer_processes
                         .iter()
                         .map(|(k, (_, v))| (*k, v.clone()))
                         .collect::<HashMap<_, _>>(), HashMap::new(), f);
        // self.network_layer_processes.entry(id).or_insert_with(|| {
        //     let (tx_down, rx_down) = flume::unbounded();
        //     let (tx_up, rx_up) = flume::unbounded();
        //     let downlink = ChassisInProcessLink {
        //         rx: rx_down,
        //         tx: self
        //             .link_layer_processes
        //             .iter()
        //             .map(|(k, (_, v))| (*k, v.clone()))
        //             .collect::<HashMap<_, _>>(),
        //     };

        //     let uplink = ChassisInProcessLink {
        //         rx: rx_up,
        //         tx: HashMap::new(),
        //     };
        //     for (_, sender) in self.link_layer_processes.values() {
        //         let _ = sender.send(ProcessMessage::NewConn(id, tx_down.clone()));
        //     }
        //     // TODO UpLink
        //     let handle = tokio::spawn(f(downlink, uplink));
        //     (handle, tx_down, tx_up)
        // });
    }
}

fn add_mid_level_process<Id, UpId, DownId, DownPayload, UpPayload, F, Fut>(
    id: Id,
    curr_level: &mut HashMap<Id, MidLayerProcessHandle<DownId, Id, UpId, DownPayload, UpPayload>>,
    down_map: HashMap<DownId, Sender<ProcessMessage<Id, DownId, DownPayload>>>,
    up_map: HashMap<UpId, Sender<ProcessMessage<Id, UpId, UpPayload>>>,
    f: F
) where
    Id: Clone + Eq + Hash,
    F: FnOnce(ChassisProcessLink<Id, DownId, DownPayload>, ChassisProcessLink<Id, UpId, UpPayload>) -> Fut,
    Fut: std::future::Future<Output = ()> + Send + 'static,
{
    curr_level.entry(id.clone()).or_insert_with(|| {
        let (tx_down, rx_down) = flume::unbounded();
        let (tx_up, rx_up) = flume::unbounded();
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
        (handle, tx_down, tx_up)
    });
}

pub type ChassisProcessLink<Id, LinkedId, Payload> = ChassisInProcessLink<LinkedId, ProcessMessage<LinkedId, Id, Payload>, ProcessMessage<Id, LinkedId, Payload>>;

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
