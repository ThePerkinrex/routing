use std::{collections::HashMap, fmt::Display, hash::Hash, sync::Arc};

use async_trait::async_trait;
use derivative::Derivative;
use either::Either;
use flume::{Receiver, RecvError, Sender};
use tokio::task::{JoinHandle, JoinSet};
use tracing::{trace, warn};

use crate::{
    either::ThreeWayEither,
    ethernet::{ethertype::EtherType, nic::Nic, packet::EthernetPacket},
    ipv4::addr::IpV4Addr,
    mac::Mac,
};

#[derive(Debug, Clone, Copy, Eq, Derivative)]
#[derivative(Hash, PartialEq)]
pub enum LinkLayerId {
    Ethernet(
        u16,
        #[derivative(PartialEq = "ignore")]
        #[derivative(Hash = "ignore")]
        Mac,
    ),
}

impl Display for LinkLayerId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Ethernet(id, _) => write!(f, "eth{id}"),
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
    Icmp,
}

pub enum ProcessMessage<SenderId, ReceiverId, Payload> {
    NewConn(
        SenderId,
        Sender<ProcessMessage<ReceiverId, SenderId, Payload>>,
    ),
    Message(SenderId, Payload),
}

pub type LinkNetworkPayload = (Mac, Vec<u8>);
pub type NetworkTransportPayload = NetworkTransportMessage;

#[derive(Debug, Clone, PartialEq, Eq)]
#[non_exhaustive]
pub enum NetworkTransportMessage {
    IPv4(IpV4Addr, Vec<u8>),
    // IPv6(IpV6Addr, Vec<u8>),
}

type LinkLayerProcessHandle = (
    JoinHandle<()>,
    Sender<ProcessMessage<NetworkLayerId, LinkLayerId, LinkNetworkPayload>>,
);

type MidLayerProcessHandle<DownId, Id, UpId, DownPayload, UpPayload> = (
    JoinHandle<()>,
    Sender<ProcessMessage<DownId, Id, DownPayload>>,
    Sender<ProcessMessage<UpId, Id, UpPayload>>,
);

pub struct NicHandle {
    connected: bool,
    disconnect: (Sender<()>, Receiver<()>),
    connect: (Sender<()>, Receiver<(barrage::Sender<EthernetPacket>, barrage::Receiver<EthernetPacket>)>),
    connect_to_net: (Sender<(barrage::Sender<EthernetPacket>, barrage::Receiver<EthernetPacket>)>, Receiver<()>),
}

impl NicHandle {
    pub async fn connect_nic(&mut self, nic: &Nic) -> Option<Nic> {
        if nic.is_up() {
            warn!("Didnt connect NIC");
            None
        } else if self.connected {
            self.connected = true;
            self.connect.0.send_async(()).await.ok()?;
            self.connect.1.recv_async().await.ok().map(|conn| Nic::join(Some(conn), nic.mac()))
        }else{
            warn!("Didnt connect NIC");
            None
        }
    }

    async fn get_connection_self(&self) -> Option<(barrage::Sender<EthernetPacket>, barrage::Receiver<EthernetPacket>)> {
        if self.connected{
            self.connect.0.send_async(()).await.ok()?;
            self.connect.1.recv_async().await.ok()
        }else{
            None
        }
    }

    async fn set_connection_self(&mut self, conn: (barrage::Sender<EthernetPacket>, barrage::Receiver<EthernetPacket>)) -> Option<()> {
        if !self.connected{
            self.connect_to_net.0.send_async(conn).await.ok()?;
            self.connect_to_net.1.recv_async().await.ok()?;
            self.connected = true;
            Some(())
        }else{
            None
        }
    }

    pub async fn connect_other(&mut self, other: &mut Self) -> bool {
        match (self.connected, other.connected) {
            (true, false) => {
                if let Some(conn) = self.get_connection_self().await {
                    other.set_connection_self(conn).await.is_some()
                }else{
                    false
                }
            }
            (false, true) => {
                if let Some(conn) = other.get_connection_self().await {
                    self.set_connection_self(conn).await.is_some()
                }else{
                    false
                }
            }
            (false, false) => {
                let conn = barrage::unbounded();
                self.set_connection_self(conn.clone()).await.is_some() && other.set_connection_self(conn).await.is_some()
            }
            (true, true) => {
                false
            }
        }
    }

    pub async fn disconnect(&mut self) -> bool {
        if self.connected {
            if self.disconnect.0.send_async(()).await.is_err() {
                return false;
            }
            let res = self.disconnect.1.recv_async().await.is_ok();
            self.connected = false;
            res
        } else {
            false
        }
    }
}

impl Display for NicHandle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "state: {}", if self.connected {"UP"} else {"DOWN"})
    }
}

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
            NetworkTransportPayload,
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

    pub fn add_nic_with_id(&mut self, id: u16, nic: Nic) -> NicHandle {
        let id = LinkLayerId::Ethernet(id, nic.mac());
        let (conn_tx, conn_rx) = flume::unbounded();
        let (dconn_tx, dconn_rx) = flume::unbounded();
        let (conn_net_tx, conn_net_rx) = flume::unbounded();
        let (conn_reply_tx, conn_reply_rx) = flume::unbounded();
        let (dconn_reply_tx, dconn_reply_rx) = flume::unbounded();
        let (conn_net_reply_tx, conn_net_reply_rx) = flume::unbounded();
        let res = NicHandle {
            connected: nic.is_up(),
            disconnect: (dconn_tx, dconn_reply_rx),
            connect: (conn_tx, conn_reply_rx),
            connect_to_net: (conn_net_tx, conn_net_reply_rx),
        };
        self.add_link_layer_process(id, move |mut up_link| async move {
            let (mut conn, addr) = nic.split();
            let dconn_rx = Arc::new(dconn_rx);
            let uplink_rx = Arc::new(up_link.rx);
            let conn_rx = Arc::new(conn_rx);
            'state_change: loop {
                if let Some((tx, rx)) = conn.take() {
                    let rx = Arc::new(rx);
                    let mut join_set = JoinSet::new();
                    // let nic_ref = &nic;
                    let rx_clone = rx.clone();
                    join_set.spawn(async move { ThreeWayEither::A(rx_clone.recv_async().await) });
                    let uplink_rx_clone = uplink_rx.clone();
                    join_set.spawn(async move {
                        ThreeWayEither::B(uplink_rx_clone.recv_async().await)
                    });
                    let dconn_rx_clone = dconn_rx.clone();
                    join_set.spawn(async move {
                        ThreeWayEither::C(Either::Right(dconn_rx_clone.recv_async().await))
                    });
                    let conn_rx_clone = conn_rx.clone();
                    join_set.spawn(async move {
                        ThreeWayEither::C(Either::Left(conn_rx_clone.recv_async().await))
                    });
                    loop {
                        match join_set.join_next().await {
                            Some(Ok(ThreeWayEither::A(eth_packet))) => {
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
                                                EtherType::IP_V4 => {
                                                    if let Some(sender) = up_link.tx.get(&NetworkLayerId::Ipv4) {
                                                        let _ = sender.send_async(ProcessMessage::Message(id, (eth_packet.get_source(), eth_packet.payload))).await.map_err(|e| warn!("Cant send ipv4 packet up: {e:?}"));
                                                    }else{
                                                        warn!(NIC = ?addr,"No IPv4 process to send packet")
                                                    }
                                                }
                                                EtherType::ARP => {
                                                    if let Some(sender) = up_link.tx.get(&NetworkLayerId::Arp) {
                                                        let _ = sender.send_async(ProcessMessage::Message(id, (eth_packet.get_source(), eth_packet.payload))).await.map_err(|e| warn!("Cant send arp packet up: {e:?}"));
                                                    }else{
                                                        warn!(NIC = ?addr,"No IPv4 process to send packet")
                                                    }
                                                }
                                                x => warn!(NIC = ?addr, "Unknown ether_type {:x}", x.to_u16())
                                            }
                                        }
                                    }
                                }
                                let rx_clone = rx.clone();
                                join_set.spawn(async move { ThreeWayEither::A(rx_clone.recv_async().await) });
                            }
                            Some(Ok(ThreeWayEither::B(up_link_msg))) => {
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
                                                trace!(NIC = ?addr, "Transmitting ARP packet");
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
                                let uplink_rx_clone = uplink_rx.clone();
                                join_set.spawn(async move {
                                    ThreeWayEither::B(uplink_rx_clone.recv_async().await)
                                });
                            }
                            Some(Ok(ThreeWayEither::C(Either::Right(msg)))) => {
                                match msg {
                                    Ok(()) => {
                                        dconn_reply_tx.send_async(()).await.unwrap();
                                        continue 'state_change
                                    },
                                    Err(e) => warn!(NIC = ?addr, "Disconnect packet error: {e:?}"),
                                }
                            }
                            Some(Ok(ThreeWayEither::C(Either::Left(msg)))) => {
                                match msg {
                                    Ok(()) => {
                                        conn_reply_tx.send_async((tx, rx.as_ref().clone())).await.unwrap();
                                        continue 'state_change
                                    },
                                    Err(e) => warn!(NIC = ?addr, "Connect packet error: {e:?}"),
                                }
                            }
                            Some(Err(e)) => warn!(NIC = ?addr, "join error: {e:?}"),
                            None => (),
                        }
                        tokio::task::yield_now().await
                    }
                }else if let Ok(conn_b) = conn_net_rx.recv_async().await {
                    conn = Some(conn_b);
                    conn_net_reply_tx.send_async(()).await.unwrap();
                    continue 'state_change;
                }
                break
            }
        });
        res
    }

    pub fn add_network_layer_process<
        P: MidLevelProcess<
                NetworkLayerId,
                TransportLayerId,
                LinkLayerId,
                LinkNetworkPayload,
                NetworkTransportPayload,
            > + Send
            + 'static,
    >(
        &mut self,
        id: NetworkLayerId,
        process: P,
    ) -> Sender<ProcessMessage<TransportLayerId, NetworkLayerId, NetworkTransportPayload>> {
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
            join_set.spawn(async move {
                ThreeWayEither::A((down_link.rx.recv_async().await, down_link.rx))
            });
            join_set.spawn(async move {
                ThreeWayEither::B((up_link.rx.recv_async().await, up_link.rx))
            });
            process.setup(&mut join_set).await;
            loop {
                match join_set.join_next().await {
                    Some(Ok(ThreeWayEither::A((down_packet, rx)))) => {
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
                        join_set
                            .spawn(async move { ThreeWayEither::A((rx.recv_async().await, rx)) });
                    }
                    Some(Ok(ThreeWayEither::B((up_link_msg, rx)))) => {
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
                        join_set
                            .spawn(async move { ThreeWayEither::B((rx.recv_async().await, rx)) });
                    }
                    Some(Ok(ThreeWayEither::C(msg))) => {
                        process
                            .on_extra_message(msg, &down_link.tx, &up_link.tx, &mut join_set)
                            .await;
                    }
                    Some(Err(e)) => warn!("join error: {e:?}"),
                    None => (),
                }
                tokio::task::yield_now().await
            }
        })
    }
}

pub type ReceptionResult<Message> = (Result<Message, RecvError>, Receiver<Message>);

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
    async fn setup(
        &mut self,
        join_set: &mut JoinSet<
            ThreeWayEither<
                ReceptionResult<ProcessMessage<DownId, Id, DownPayload>>,
                ReceptionResult<ProcessMessage<UpId, Id, UpPayload>>,
                Self::Extra,
            >,
        >,
    ) {
    }
    type Extra: Send;
    async fn on_extra_message(
        &mut self,
        msg: Self::Extra,
        down_sender: &HashMap<DownId, Sender<ProcessMessage<Id, DownId, DownPayload>>>,
        up_sender: &HashMap<UpId, Sender<ProcessMessage<Id, UpId, UpPayload>>>,
        join_set: &mut JoinSet<
            ThreeWayEither<
                ReceptionResult<ProcessMessage<DownId, Id, DownPayload>>,
                ReceptionResult<ProcessMessage<UpId, Id, UpPayload>>,
                Self::Extra,
            >,
        >,
    ) {
    }
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
    type Extra = ();
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
