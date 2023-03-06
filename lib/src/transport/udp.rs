use std::{collections::HashMap, future::Future, pin::Pin, sync::Arc};

use either::Either;
use flume::{Receiver, RecvError, Sender};
use tokio::task::JoinSet;

use futures::FutureExt;
use tracing::{trace, warn};

use crate::{
    chassis::{
        NetworkLayerId, NetworkTransportMessage, ProcessMessage, TransportLayerId,
        TransportLevelProcess,
    },
    network::ipv4::addr::IpV4Addr,
};

use self::packet::UdpPacket;

pub mod packet;

// TODO Build process with API
// TODO Socket

type Duplex<T, R> = (Sender<T>, Arc<Receiver<R>>);

fn new_pair<A, B>() -> (Duplex<A, B>, Duplex<B, A>) {
    let a = flume::unbounded();
    let b = flume::unbounded();
    ((a.0, Arc::new(b.1)), (b.0, Arc::new(a.1)))
}

type Data<Addr> = (Addr, u16, Vec<u8>, Option<u8>);

pub struct Socket<Addr> {
    duplex: Duplex<Data<Addr>, Data<Addr>>,
}

impl<Addr: Send> Socket<Addr> {
    pub async fn send(&self, dest: (Addr, u16), payload: Vec<u8>) {
        self.send_ttl_internal(dest, payload, None).await
    }

    pub async fn send_ttl(&self, dest: (Addr, u16), payload: Vec<u8>, ttl: u8) {
        self.send_ttl_internal(dest, payload, Some(ttl)).await
    }

    async fn send_ttl_internal(
        &self,
        (addr, port): (Addr, u16),
        payload: Vec<u8>,
        ttl: Option<u8>,
    ) {
        if self
            .duplex
            .0
            .send_async((addr, port, payload, ttl))
            .await
            .is_err()
        {
            warn!("UDP Packet not sent");
        }
    }

    pub async fn recv(&self) -> Result<Data<Addr>, RecvError> {
        self.duplex.1.recv_async().await
    }
}

struct SocketController<Addr> {
    map: HashMap<u16, Duplex<Data<Addr>, Data<Addr>>>,
}

impl<Addr> SocketController<Addr> {
    fn new() -> Self {
        Self {
            map: HashMap::new(),
        }
    }

    fn add_socket(&mut self, port: u16) -> (Socket<Addr>, Arc<Receiver<Data<Addr>>>) {
        let (internal, external) = new_pair();
        let rx = internal.1.clone();
        self.map.insert(port, internal);
        (Socket { duplex: external }, rx)
    }
}

impl<Addr> Default for SocketController<Addr> {
    fn default() -> Self {
        Self::new()
    }
}

pub struct UdpHandleGeneric<Addr> {
    add_socket: Sender<(u16, tokio::sync::oneshot::Sender<Socket<Addr>>)>,
}

impl<Addr: Send> UdpHandleGeneric<Addr> {
    pub async fn get_socket(&self, port: u16) -> Result<Socket<Addr>, ()> {
        let (tx, rx) = tokio::sync::oneshot::channel();
        self.add_socket
            .send_async((port, tx))
            .await
            .map_err(|_| ())?;
        rx.await.map_err(|_| ())
    }
}

pub struct UdpProcessGeneric<Addr> {
    sockets: SocketController<Addr>,
    add_socket: Arc<Receiver<(u16, tokio::sync::oneshot::Sender<Socket<Addr>>)>>,
}

impl<Addr> UdpProcessGeneric<Addr> {
    pub fn new() -> (Self, UdpHandleGeneric<Addr>) {
        let (tx, rx) = flume::unbounded();
        (
            Self {
                sockets: SocketController::new(),
                add_socket: Arc::new(rx),
            },
            UdpHandleGeneric { add_socket: tx },
        )
    }
}
pub enum ExtraMessageGeneric<Addr> {
    SocketMessage(u16, Result<Data<Addr>, RecvError>),
    AddSocket(Result<(u16, tokio::sync::oneshot::Sender<Socket<Addr>>), RecvError>),
}

#[async_trait::async_trait]
trait TransportLevelComposableProcess {
    type Extra: Send + 'static;
    type Addr;
    type DownPayload;
    type DownId;

    async fn setup<F: FnMut(Pin<Box<dyn Future<Output = Self::Extra> + Send>>) + Send>(
        &mut self,
        _add_receiver: F,
    ) {
    }

    async fn on_extra<
        F: Fn(Self::Addr, Vec<u8>, Option<u8>) -> Fut + Send + Sync,
        Fut: Future<Output = ()> + Send,
    >(
        &mut self,
        _msg: Self::Extra,
        _send_down: F,
    ) -> Vec<Pin<Box<dyn Future<Output = Self::Extra> + Send>>> {
        vec![]
    }

    async fn on_down_message<
        F: Fn(Self::Addr, Vec<u8>, Option<u8>) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    >(
        &mut self,
        id: Self::DownId,
        msg: Self::DownPayload,
        send_down: F,
    );
}

#[async_trait::async_trait]
impl<Addr> TransportLevelComposableProcess for UdpProcessGeneric<Addr>
where
    Addr: Send + 'static,
    Addr: std::fmt::Debug,
{
    type Extra = ExtraMessageGeneric<Addr>;
    type Addr = Addr;
    type DownPayload = (Self::Addr, Vec<u8>, Option<u8>);
    type DownId = ();

    async fn setup<F: FnMut(Pin<Box<dyn Future<Output = Self::Extra> + Send>>) + Send>(
        &mut self,
        mut add_receiver: F,
    ) {
        let rx = self.add_socket.clone();
        add_receiver(async move { ExtraMessageGeneric::AddSocket(rx.recv_async().await) }.boxed());
        for (&port, (_, rx)) in self.sockets.map.iter() {
            let rx = rx.clone();
            add_receiver(
                async move { ExtraMessageGeneric::SocketMessage(port, rx.recv_async().await) }
                    .boxed(),
            )
        }
    }

    async fn on_extra<
        F: Fn(Self::Addr, Vec<u8>, Option<u8>) -> Fut + Send + Sync,
        Fut: Future<Output = ()> + Send,
    >(
        &mut self,
        msg: Self::Extra,
        send_down: F,
    ) -> Vec<Pin<Box<dyn Future<Output = Self::Extra> + Send>>> {
        match msg {
            ExtraMessageGeneric::AddSocket(r) => match r {
                Ok((port, sender)) => {
                    let (socket, rx2) = self.sockets.add_socket(port);
                    let _ = sender.send(socket);
                    let rx = self.add_socket.clone();
                    vec![
                        async move { ExtraMessageGeneric::AddSocket(rx.recv_async().await) }
                            .boxed(),
                        async move { ExtraMessageGeneric::SocketMessage(port, rx2.recv_async().await) }.boxed()
                    ]
                }
                Err(RecvError::Disconnected) => {
                    warn!("Add socket disconnected");
                    vec![]
                }
            },
            ExtraMessageGeneric::SocketMessage(port, r) => match r {
                Ok((dest_addr, dest_port, payload, ttl)) => {
                    trace!("Sending udp packet to {dest_addr:?}:{dest_port} with payload: {payload:?} (ttl={ttl:?})");
                    send_down(
                        dest_addr,
                        UdpPacket {
                            source_port: port,
                            destination_port: dest_port,
                            payload,
                        }
                        .to_vec(),
                        ttl,
                    )
                    .await;
                    self.sockets
                        .map
                        .get(&port)
                        .map(|(_, rx)| rx.clone())
                        .map(|rx| {
                            vec![async move {
                                ExtraMessageGeneric::SocketMessage(port, rx.recv_async().await)
                            }
                            .boxed()]
                        })
                        .unwrap_or_default()
                }
                Err(RecvError::Disconnected) => {
                    // warn!("Socket message for port {port} disconnected");
                    vec![]
                }
            },
        }
    }

    async fn on_down_message<
        F: Fn(Self::Addr, Vec<u8>, Option<u8>) -> Fut + Send,
        Fut: Future<Output = ()> + Send,
    >(
        &mut self,
        _id: Self::DownId,
        (addr, msg, ttl): Self::DownPayload,
        _send_down: F,
    ) {
        if let Some(packet) = UdpPacket::from_vec(&msg) {
            if let Some((tx, _)) = self.sockets.map.get(&packet.destination_port).cloned() {
                if tx
                    .send_async((addr, packet.source_port, packet.payload, ttl))
                    .await
                    .is_err()
                {
                    self.sockets.map.remove(&packet.destination_port);
                }
            }
        }
    }
}

pub struct UdpProcess {
    ip_v4: UdpProcessGeneric<IpV4Addr>,
}

impl UdpProcess {
    pub const fn new(ip_v4: UdpProcessGeneric<IpV4Addr>) -> Self {
        Self { ip_v4 }
    }
}

pub enum ExtraMessage {
    IPv4(ExtraMessageGeneric<IpV4Addr>),
}

#[async_trait::async_trait]
impl TransportLevelProcess<TransportLayerId, NetworkLayerId, NetworkTransportMessage>
    for UdpProcess
{
    async fn on_down_message(
        &mut self,
        msg: NetworkTransportMessage,
        down_id: NetworkLayerId,
        down_sender: &HashMap<
            NetworkLayerId,
            Sender<ProcessMessage<TransportLayerId, NetworkLayerId, NetworkTransportMessage>>,
        >,
    ) {
        if let (NetworkLayerId::Ipv4, NetworkTransportMessage::IPv4(addr, ttl, payload)) =
            (down_id, msg)
        {
            self.ip_v4
                .on_down_message((), (addr, payload, ttl), |addr, payload, ttl| async move {
                    if let Some(tx) = down_sender.get(&NetworkLayerId::Ipv4) {
                        let _ = tx
                            .send_async(ProcessMessage::Message(
                                TransportLayerId::Udp,
                                NetworkTransportMessage::IPv4(addr, ttl, payload),
                            ))
                            .await;
                    }
                })
                .await;
        }
    }
    async fn setup(
        &mut self,
        join_set: &mut JoinSet<
            Either<
                Result<
                    ProcessMessage<NetworkLayerId, TransportLayerId, NetworkTransportMessage>,
                    RecvError,
                >,
                Self::Extra,
            >,
        >,
    ) {
        self.ip_v4
            .setup(|fut| {
                join_set.spawn(async move { Either::Right(ExtraMessage::IPv4(fut.await)) });
            })
            .await;
    }
    type Extra = ExtraMessage;
    async fn on_extra_message(
        &mut self,
        msg: Self::Extra,
        down_sender: &HashMap<
            NetworkLayerId,
            Sender<ProcessMessage<TransportLayerId, NetworkLayerId, NetworkTransportMessage>>,
        >,
        join_set: &mut JoinSet<
            Either<
                Result<
                    ProcessMessage<NetworkLayerId, TransportLayerId, NetworkTransportMessage>,
                    RecvError,
                >,
                Self::Extra,
            >,
        >,
    ) {
        match msg {
            ExtraMessage::IPv4(msg) => {
                for r in self
                    .ip_v4
                    .on_extra(msg, |addr, payload, ttl| async move {
                        if let Some(tx) = down_sender.get(&NetworkLayerId::Ipv4) {
                            let _ = tx
                                .send_async(ProcessMessage::Message(
                                    TransportLayerId::Udp,
                                    NetworkTransportMessage::IPv4(addr, ttl, payload),
                                ))
                                .await;
                        }
                    })
                    .await
                {
                    join_set.spawn(async move { Either::Right(ExtraMessage::IPv4(r.await)) });
                }
            }
        }
    }
}
