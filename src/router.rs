use std::{fmt::Display, ops::BitAnd};

use barrage::SendError;
use tracing::{debug, error, trace, warn};

use crate::{
    ethernet::{EthernetLink, PhysicalAddr},
    ip::Ip,
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

    pub fn start(self) -> (flume::Sender<()>, std::thread::JoinHandle<Self>) {
        let (tx, rx) = flume::bounded(1);
        (
            tx,
            std::thread::spawn(move || {
                let router = self;
                loop {
                    if rx.try_recv() == Ok(()) {
                        break router;
                    }
                    for link in &router.interfaces {
                        if let Ok(Some(packet)) = link.try_recv() {
                            if &packet.dest == &router.ip {
                                debug!(
                                    "Packet recieved at ip {}: {:?}",
                                    router.ip,
                                    packet.payload.as_bytes()
                                )
                            } else if let Some((interface, addr)) = router.route(&packet.dest) {
                                trace!(
                                    "Routing packet from {} going to {} though if {} [{}]",
                                    packet.origin,
                                    packet.dest,
                                    interface,
                                    addr
                                );
                                if let Err(SendError(packet)) =
                                    router.interfaces[interface].send(packet, addr)
                                {
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
                }
            }),
        )
    }

    fn route(&self, addr: &IpAddr) -> Option<(InterfaceId, PhysicalAddr)> {
        self.routing_table
            .iter()
            .find(|(dest, mask, _, _)| addr & mask == dest & mask)
            .map(|(_, _, i, a)| (*i, *a))
    }
}
