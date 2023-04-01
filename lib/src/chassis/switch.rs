use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};

use barrage::Receiver;
use flume::RecvError;
use tokio::{
    sync::RwLock,
    task::{JoinHandle, JoinSet},
};
use tracing::{info, warn};

use crate::{
    link::ethernet::{dot1q::Tag, nic::Nic, packet::EthernetPacket},
    mac::{authority::MacAdminAuthority, Mac},
};

use super::NicHandle;

enum SwitchMessage {
    Connect,
    Disconnect,
    ConnectNetwork(
        (
            barrage::Sender<EthernetPacket>,
            barrage::Receiver<EthernetPacket>,
        ),
    ),
    NicHandleError(flume::RecvError),
    EthernetFrame(EthernetPacket),
    SendFrame(EthernetPacket),
    NetError(barrage::Disconnected),
}

#[derive(Debug, Clone, Copy)]
pub enum PortType {
    Trunk,
    Unknown,
    NoDot1q,
    Vlan(Tag),
}

struct Port {
    _handle: JoinHandle<()>,
    nic_handle: Arc<RwLock<NicHandle>>,
    port_type: PortType,
    sender: flume::Sender<EthernetPacket>,
}

#[derive(Default)]
pub struct Switch {
    link_layer_processes: Arc<RwLock<Vec<Port>>>,
    destination_if_table: Arc<RwLock<HashMap<Mac, (usize, Instant)>>>,
    destination_ttl: Arc<Duration>,
}

impl Switch {
    pub fn new(ttl: Duration) -> Self {
        Self {
            destination_ttl: Arc::new(ttl),
            ..Default::default()
        }
    }

    fn internal_clone(&self) -> Self {
        Self {
            link_layer_processes: self.link_layer_processes.clone(),
            destination_if_table: self.destination_if_table.clone(),
            destination_ttl: self.destination_ttl.clone(),
        }
    }

    async fn send_frame(&self, frame: EthernetPacket) {
        let dest = frame.get_dest();
        if let Some(tag) = frame.get_dot1q() {
            match match self.destination_if_table.read().await.get(&dest).copied() {
                Some((_, created)) if &created.elapsed() > self.destination_ttl.as_ref() => {
                    self.destination_if_table.write().await.remove(&dest);
                    None
                }
                Some((id, cr)) => match self.link_layer_processes.read().await[id].port_type {
                    PortType::Vlan(tag2) if tag.vlan_id() != tag2.vlan_id() => {
                        self.destination_if_table.write().await.remove(&dest);
                        None
                    } // Saved if doesnt match vlan
                    _ => Some((id, cr)),
                },
                x => x,
            } {
                Some((id, _)) => {
                    let _ = self.link_layer_processes.read().await[id]
                        .sender
                        .send_async(frame)
                        .await;
                }
                None => {
                    let llp = self.link_layer_processes.read().await;
                    for Port {
                        port_type, sender, ..
                    } in llp.iter()
                    {
                        if match port_type {
                            PortType::Trunk => true,
                            PortType::Vlan(vlan) if vlan.vlan_id() == tag.vlan_id() => true,
                            _ => false,
                        } {
                            let _ = sender.send_async(frame.clone()).await;
                        }
                    }
                }
            }
        } else {
            // nodot1q
            match match self.destination_if_table.read().await.get(&dest).copied() {
                Some((_, created)) if &created.elapsed() > self.destination_ttl.as_ref() => {
                    self.destination_if_table.write().await.remove(&dest);
                    None
                }
                x => x,
            } {
                Some((id, _)) => {
                    let _ = self.link_layer_processes.read().await[id]
                        .sender
                        .send_async(frame)
                        .await;
                }
                None => {
                    let llp = self.link_layer_processes.read().await;
                    for Port {
                        port_type, sender, ..
                    } in llp.iter()
                    {
                        if matches!(port_type, PortType::NoDot1q | PortType::Unknown) {
                            let _ = sender.send_async(frame.clone()).await;
                        }
                    }
                }
            }
        }
    }

    pub async fn add_nic(&self, nic: Nic, t: PortType) -> (usize, Arc<RwLock<NicHandle>>) {
        let (conn_tx, conn_rx) = flume::unbounded();
        let (dconn_tx, dconn_rx) = flume::unbounded();
        let (conn_net_tx, conn_net_rx) = flume::unbounded();
        let (conn_reply_tx, conn_reply_rx) = flume::unbounded();
        let (dconn_reply_tx, dconn_reply_rx) = flume::unbounded();
        let (conn_net_reply_tx, conn_net_reply_rx) = flume::unbounded();
        let (frame_tx, frame_rx) = flume::unbounded();
        let res = (
            self.link_layer_processes.read().await.len(),
            Arc::new(RwLock::new(NicHandle {
                connected: nic.is_up(),
                disconnect: (dconn_tx, dconn_reply_rx),
                connect: (conn_tx, conn_reply_rx),
                connect_to_net: (conn_net_tx, conn_net_reply_rx),
            })),
        );
        let id = res.0;
        let self_inner = self.internal_clone();
        self.link_layer_processes.write().await.push(Port {
            _handle: tokio::spawn(async move {
                let conn_rx = Arc::new(conn_rx);
                let conn_task = move || {
                    let rx = conn_rx.clone();
                    async move {
                        rx.recv_async()
                            .await
                            .map_or_else(SwitchMessage::NicHandleError, |()| SwitchMessage::Connect)
                    }
                };
                let dconn_rx = Arc::new(dconn_rx);
                let dconn_task = move || {
                    let rx = dconn_rx.clone();
                    async move {
                        rx.recv_async()
                            .await
                            .map_or_else(SwitchMessage::NicHandleError, |()| {
                                SwitchMessage::Disconnect
                            })
                    }
                };
                let conn_net_rx = Arc::new(conn_net_rx);
                let conn_net_task = move || {
                    let rx = conn_net_rx.clone();
                    async move {
                        rx.recv_async().await.map_or_else(
                            SwitchMessage::NicHandleError,
                            SwitchMessage::ConnectNetwork,
                        )
                    }
                };
                let frame_rx = Arc::new(frame_rx);
                let frame_task = move || {
                    let rx = frame_rx.clone();
                    async move {
                        SwitchMessage::SendFrame(rx.recv_async().await.unwrap())
                    }
                };
                let ethernet_task = |rx: Arc<Receiver<EthernetPacket>>| async move {
                    rx.recv_async()
                        .await
                        .map_or_else(SwitchMessage::NetError, SwitchMessage::EthernetFrame)
                };
                let (conn, _) = nic.split();
                let mut join_set = JoinSet::new();
                join_set.spawn(dconn_task());
                join_set.spawn(conn_net_task());
                let mut conn = conn.map(|(tx, rx)| (tx, Arc::new(rx)));
                if let Some((_, rx)) = conn.as_ref() {
                    join_set.spawn(conn_task());
                    join_set.spawn(ethernet_task(rx.clone()));
                    join_set.spawn(frame_task());
                }
                loop {
                    match join_set.join_next().await {
                        Some(Ok(x)) => match x {
                            SwitchMessage::Connect => if let Some((tx, rx)) = conn.as_ref() {
                                let _ = conn_reply_tx
                                    .send_async((tx.clone(), rx.as_ref().clone()))
                                    .await;
                                join_set.spawn(conn_task());
                            },
                            SwitchMessage::Disconnect => {
                                conn = None;
                                let _ = dconn_reply_tx.send_async(()).await;
                                join_set.spawn(dconn_task());
                            }
                            SwitchMessage::ConnectNetwork((tx, rx)) => {
                                let rx = Arc::new(rx);
                                conn = Some((tx, rx.clone()));
                                let _ = conn_net_reply_tx.send_async(()).await;
                                join_set.spawn(conn_net_task());
                                join_set.spawn(ethernet_task(rx));
                                join_set.spawn(frame_task());
                            }
                            SwitchMessage::NicHandleError(RecvError::Disconnected) => {
                                warn!("NIC handle disconnected")
                            }
                            SwitchMessage::EthernetFrame(mut frame) => {
                                self_inner.destination_if_table.write().await.insert(frame.get_source(), (id, Instant::now()));
								let port_state = self_inner.link_layer_processes.read().await[id].port_type;
								match (port_state, frame.get_dot1q()) {
									(PortType::Trunk, None) => warn!("Recieved non baby jumbo frame from trunk configured port eth{id}"),
									(PortType::Trunk, Some(_)) => {
										self_inner.send_frame(frame).await;
									},
									(PortType::Unknown, None) => {
										info!("[eth{id}] Received normal frame from unknown state port, treating as no dot1q, no info on port");
										self_inner.send_frame(frame).await;
									},
									(PortType::Unknown, Some(_)) => {
										info!("[eth{id}] Received baby jumbo from unknown state port, configuring as trunk");
										self_inner.link_layer_processes.write().await[id].port_type = PortType::Trunk;
										self_inner.send_frame(frame).await;
									},
									(PortType::NoDot1q, None) => {
										self_inner.send_frame(frame).await;
									},
									(PortType::NoDot1q, Some(tag)) => warn!(?tag, "Recieved baby jumbo frame from no dot1q configured port eth{id}"),
									(PortType::Vlan(vlan_id), None) => {
										frame.set_dot1q(vlan_id);
										self_inner.send_frame(frame).await;
									},
									(PortType::Vlan(vlan_id), Some(tag)) => warn!(?tag, ?vlan_id, "Recieved baby jumbo frame from no vlan endpoint configured port eth{id}"),
								}
							}
                            SwitchMessage::NetError(_) => todo!(),
                            SwitchMessage::SendFrame(mut frame) => if let Some((tx, _)) = conn.as_ref() {
								let port_state = self_inner.link_layer_processes.read().await[id].port_type;
                                match (port_state, frame.get_dot1q()) {
									(PortType::Trunk, None) => warn!("Not sent non baby jumbo frame to trunk configured port eth{id}"),
									(PortType::Trunk, Some(_)) => {
										let _ = tx.send_async(frame).await;
									},
									(PortType::Unknown, None) => {
										info!("[eth{id}] Sent normal frame from unknown state port, treating as no dot1q, no info on port");
										let _ = tx.send_async(frame).await;
									},
									(PortType::Unknown, Some(_)) => {
										info!("[eth{id}] Sent baby jumbo from unknown state port, configuring as trunk");
										self_inner.link_layer_processes.write().await[id].port_type = PortType::Trunk;
										let _ = tx.send_async(frame).await;
									},
									(PortType::NoDot1q, None) => {
										let _ = tx.send_async(frame).await;
									},
									(PortType::NoDot1q, Some(tag)) => warn!(?tag, "Sent baby jumbo frame to no dot1q configured port eth{id}"),
									(PortType::Vlan(vlan_id), None) => {
										warn!(vlan_id = vlan_id.vlan_id(), "[eth{id}] Dropping packet attempted to send through vlan port without tag");
									},
									(PortType::Vlan(vlan_id), Some(tag)) => {
                                        if vlan_id.vlan_id() == tag.vlan_id() {
                                            frame.remove_dot1q();
                                            let _ = tx.send_async(frame).await;
                                        }else{
										    warn!(vlan_id = vlan_id.vlan_id(), tag = tag.vlan_id(), "[eth{id}] Dropping packet attempted to send through vlan port with wrong tag");
                                        }
                                    },
								}
                            }
                        },
                        Some(Err(_)) => break,
                        None => break,
                    }
                }
            }),
            nic_handle: res.1.clone(),
            port_type: t,
            sender: frame_tx,
        });
        res
    }

    pub async fn ports_len(&self) -> usize {
        self.link_layer_processes.read().await.len()
    }

    pub async fn ports(&self) -> Vec<(usize, PortType, Arc<RwLock<NicHandle>>)> {
        self.link_layer_processes
            .read()
            .await
            .iter()
            .enumerate()
            .map(|(i, x)| (i, x.port_type, x.nic_handle.clone()))
            .collect()
    }

    pub async fn get_port_type(&self, i: usize) -> Option<PortType> {
        self.link_layer_processes
            .read()
            .await
            .get(i)
            .map(|x| x.port_type)
    }
    pub async fn set_port_type(&self, i: usize, port_type: PortType) {
        if let Some(x) = self.link_layer_processes.write().await.get_mut(i) {
            x.port_type = port_type
        }
    }
}

// #[async_trait::async_trait]
// impl crate::node::ports::Ports for Switch {
// 	type PortsIter = Box<dyn Iterator<Item = Box<dyn crate::node::ports::Port>>>;
// 	async fn ports(&self) -> Self::PortsIter {
// 		Box::new(self.ports().await.into_iter().map(|x| Box::new(x) as Box<dyn crate::node::ports::Port>))
// 	}
// }

pub async fn switch<A: MacAdminAuthority + Send>(
    ports: usize,
    mac_authority: &mut A,
    port_type: PortType,
) -> Switch {
    let s = Switch::new(Duration::from_secs(5));
    for _ in 0..ports {
        s.add_nic(Nic::new(mac_authority), port_type).await;
    }
    s
}

pub async fn simple_switch<A: MacAdminAuthority + Send>(
    ports: usize,
    mac_authority: &mut A,
) -> Switch {
    switch(ports, mac_authority, PortType::NoDot1q).await
}

pub async fn standard_switch<A: MacAdminAuthority + Send>(
    ports: usize,
    mac_authority: &mut A,
) -> Switch {
    switch(ports, mac_authority, PortType::Unknown).await
}
