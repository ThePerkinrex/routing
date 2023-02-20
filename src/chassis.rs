use std::collections::HashMap;

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

pub enum NetworkLinkMessage<SenderId, ReceiverId> {
    NewConn(SenderId, Sender<NetworkLinkMessage<ReceiverId, SenderId>>),
    Message(SenderId, Mac, Vec<u8>),
}

#[derive(Debug, Default)]
pub struct Chassis {
    link_layer_processes: HashMap<
        LinkLayerId,
        (
            JoinHandle<()>,
            Sender<NetworkLinkMessage<NetworkLayerId, LinkLayerId>>,
        ),
    >,
    network_layer_processes: HashMap<
        NetworkLayerId,
        (
            JoinHandle<()>,
            Sender<NetworkLinkMessage<LinkLayerId, NetworkLayerId>>,
        ),
    >,
}

impl Chassis {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn add_link_layer_process<
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
                    .map(|(k, (_, v))| (*k, v.clone()))
                    .collect::<HashMap<_, _>>(),
                id,
            };
            for (_, sender) in self.network_layer_processes.values() {
                let _ = sender.send(NetworkLinkMessage::NewConn(id, tx.clone()));
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
                                Ok(eth_packet) => {
                                    trace!("NIC {} recieved packet: {:?}", addr, eth_packet);
                                }
                                Err(_) => warn!("Error recieving eth packet: Disconnected"),
                            };
                            join_set
                                .spawn(async move { Either::Left((rx.recv_async().await, rx)) });
                        }
						Some(Ok(Either::Right((up_link_msg, rx)))) => {
							match up_link_msg {
								Ok(up_link_msg) => match up_link_msg {
									NetworkLinkMessage::NewConn(upper_id, sender) => {
										up_link.tx.insert(upper_id, sender);
									}
									NetworkLinkMessage::Message(id, dest, payload) => match id {
										NetworkLayerId::Ipv4 => {
											trace!("Transmitting ipv4 packet");
											match EthernetPacket::new_ip_v4(dest, addr, payload) {
												Some(packet) => {
													let _ = tx.send_async(packet).await;
												}
												None => warn!("Error building ethernet ipv4 packet"),
											}
										}
										NetworkLayerId::Ipv6 => {
											match EthernetPacket::new_ip_v6(dest, addr, payload) {
												Some(packet) => {
													let _ = tx.send_async(packet).await;
												}
												None => warn!("Error building ethernet ipv6 packet"),
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
								}
								Err(e) => warn!("Down link packet error: {e:?}"),
							}
							join_set.spawn(async move {
								Either::Right((rx.recv_async().await, rx))
							});
						}
						Some(Err(e)) => warn!("join error: {e:?}"),
						None => ()
                    }
                    tokio::task::yield_now().await
                }
            }
        })
    }

    pub fn add_network_layer_process<
        F: FnOnce(NetworkProcessDownLink) -> Fut,
        Fut: std::future::Future<Output = ()> + Send + 'static,
    >(
        &mut self,
        id: NetworkLayerId,
        f: F,
    ) {
        self.network_layer_processes.entry(id).or_insert_with(|| {
            let (tx, rx) = flume::unbounded();
            let link = ChassisInProcessLink {
                rx,
                tx: self
                    .link_layer_processes
                    .iter()
                    .map(|(k, (_, v))| (*k, v.clone()))
                    .collect::<HashMap<_, _>>(),
                id,
            };
            for (_, sender) in self.link_layer_processes.values() {
                let _ = sender.send(NetworkLinkMessage::NewConn(id, tx.clone()));
            }
            let handle = tokio::spawn(f(link));
            (handle, tx)
        });
    }
}

pub type NetworkProcessDownLink = ChassisInProcessLink<
    NetworkLayerId,
    LinkLayerId,
    NetworkLinkMessage<LinkLayerId, NetworkLayerId>,
    NetworkLinkMessage<NetworkLayerId, LinkLayerId>,
>;
pub type LinkProcessUpLink = ChassisInProcessLink<
    LinkLayerId,
    NetworkLayerId,
    NetworkLinkMessage<NetworkLayerId, LinkLayerId>,
    NetworkLinkMessage<LinkLayerId, NetworkLayerId>,
>;

pub struct ChassisInProcessLink<Id, LinkedId, RecvMsg, SendMsg> {
    pub rx: Receiver<RecvMsg>,
    pub tx: HashMap<LinkedId, Sender<SendMsg>>,
    pub id: Id,
}
