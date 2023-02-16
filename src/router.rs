use std::{fmt::Display, ops::BitAnd};

use barrage::SendError;
use flume::TryRecvError;
use tracing::{debug, error, trace, warn};

use crate::{
    ethernet::{EthernetLink, PhysicalAddr},
    ip::Ip,
    link::Link,
    tcp_ip::Payload,
};

type InterfaceId = usize;

pub struct Router<P: Payload<1480> + Clone + Unpin, IpAddr: Clone + Unpin> {
    interfaces: Vec<EthernetLink<Ip<P, IpAddr>>>,
    routing_table: Vec<(IpAddr, IpAddr, InterfaceId, PhysicalAddr)>,
    ip: IpAddr,
}

impl<
        P: Payload<1480> + Clone + Unpin + Sync + Send + 'static,
        IpAddr: Clone + Unpin + Display + Send + Sync + 'static,
    > Router<P, IpAddr>
where
    for<'a> &'a IpAddr: BitAnd + Eq,
    for<'a> <&'a IpAddr as BitAnd>::Output: Eq,
{
    pub fn new(
        interfaces: Vec<EthernetLink<Ip<P, IpAddr>>>,
        routing_table: Vec<(IpAddr, IpAddr, InterfaceId, PhysicalAddr)>,
        ip: IpAddr,
    ) -> Self {
        Self {
            interfaces,
            routing_table,
            ip,
        }
    }

    pub fn new_simple(
        gateway_link: EthernetLink<Ip<P, IpAddr>>,
        gateway_addr: PhysicalAddr,
        gateway_mask: IpAddr,
        gateway_mask_ip: IpAddr,
        ip: IpAddr,
    ) -> Self {
        Self::new(
            vec![gateway_link],
            vec![(gateway_mask_ip, gateway_mask, 0, gateway_addr)],
            ip,
        )
    }

    pub fn start(self) -> RouterHandle<P, IpAddr> {
        let (tx, rx) = flume::bounded(1);
        let (tx2, rx2) = flume::unbounded();
        RouterHandle {
            tx,
            rx: rx2,
            addr: self.ip.clone(),
            handle: std::thread::spawn(move || {
                let router = self;
                loop {
                    match rx.try_recv() {
                        Ok(RouterCommand::Stop) => break router,
                        Ok(RouterCommand::SendPacket(packet, addr)) => router.try_send_packet(Ip {
                            origin: router.ip.clone(),
                            dest: addr,
                            payload: packet,
                        }),
                        _ => (),
                    }
                    for link in &router.interfaces {
                        if let Ok(Some(packet)) = link.try_recv() {
                            if &packet.dest == &router.ip {
                                debug!(
                                    "Packet recieved at ip {}: {:?}",
                                    router.ip,
                                    packet.payload.as_bytes()
                                );
                                if tx2.send(packet).is_err() {
                                    warn!("Error sending recieved packet upwards")
                                }
                            } else {
                                router.try_send_packet(packet);
                            }
                        }
                    }
                }
            }),
        }
    }

    fn route(&self, addr: &IpAddr) -> Option<(InterfaceId, PhysicalAddr)> {
        self.routing_table
            .iter()
            .find(|(dest, mask, _, _)| addr & mask == dest & mask)
            .map(|(_, _, i, a)| (*i, *a))
    }

    fn try_send_packet(&self, packet: Ip<P, IpAddr>) {
        if let Some((interface, addr)) = self.route(&packet.dest) {
            trace!(
                "Routing packet from {} going to {} though if {} [{}]",
                packet.origin,
                packet.dest,
                interface,
                addr
            );
            if let Err(SendError(packet)) = self.interfaces[interface].send(packet, addr) {
                error!(
                    "Router send error for packet: [origin: {} dest: {}]",
                    packet.origin, packet.dest
                )
            }
        } else {
            warn!(
                "Packet can't be redirected: [origin: {} dest: {}]",
                &packet.origin, &packet.dest
            )
        }
    }
}

enum RouterCommand<P, A> {
    Stop,

    SendPacket(P, A),
}

pub struct RouterHandle<P: Payload<1480> + Clone + Unpin, IpAddr: Clone + Unpin> {
    tx: flume::Sender<RouterCommand<P, IpAddr>>,
    rx: flume::Receiver<Ip<P, IpAddr>>,
    handle: std::thread::JoinHandle<Router<P, IpAddr>>,
    addr: IpAddr,
}

impl<
        P: Payload<1480> + Clone + Unpin + Sync + Send + 'static,
        IpAddr: Clone + Unpin + Display + Send + Sync + 'static,
    > RouterHandle<P, IpAddr>
{
    pub fn stop(self) -> Result<Router<P, IpAddr>, ()> {
        self.tx.send(RouterCommand::Stop).map_err(|_| ())?;
        self.handle.join().map_err(|_| ())
    }

    // pub fn restart(self) -> Result<Self, ()>
    // where
    //     for<'a> &'a IpAddr: BitAnd + Eq,
    //     for<'a> <&'a IpAddr as BitAnd>::Output: Eq,
    // {
    //     Ok(self.stop()?.start())
    // }
}

impl<
        P: Payload<1480> + Clone + Unpin + Sync + Send + 'static,
        IpAddr: Clone + Unpin + Display + Send + Sync + 'static,
    > Link for RouterHandle<P, IpAddr>
{
    type Addr = IpAddr;

    type Packet = P;

    type RecvError = flume::RecvError;

    type SendError = SendError<()>;

    fn get_addr(&self) -> Self::Addr {
        self.addr.clone()
    }

    fn recv(&self) -> Result<Self::Packet, Self::RecvError> {
        self.rx.recv().map(|ip_packet| ip_packet.payload)
    }

    fn try_recv(&self) -> Result<Option<Self::Packet>, Self::RecvError> {
        match self.rx.try_recv().map(|ip_packet| ip_packet.payload) {
            Ok(x) => Ok(Some(x)),
            Err(TryRecvError::Empty) => Ok(None),
            Err(TryRecvError::Disconnected) => Err(flume::RecvError::Disconnected),
        }
    }

    fn send(&self, packet: Self::Packet, dest: Self::Addr) -> Result<(), Self::SendError> {
        self.tx
            .send(RouterCommand::SendPacket(packet, dest))
            .map_err(|_| SendError(()))
    }
}
