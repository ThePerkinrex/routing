use barrage::SendError;

use crate::{
    ethernet::{EthernetLink, PhysicalAddr},
    tcp_ip::{Ip, IpAddr, Payload},
};

pub type IpMask = IpAddr;
type InterfaceId = usize;

pub struct Router<P: Payload<1480> + Clone + Unpin> {
    interfaces: Vec<EthernetLink<Ip<P>>>,
    routing_table: Vec<(IpAddr, IpMask, InterfaceId, PhysicalAddr)>,
}

impl<P: Payload<1480> + Clone + Unpin + Sync + Send + 'static> Router<P> {
    pub fn new(
        interfaces: Vec<EthernetLink<Ip<P>>>,
        routing_table: Vec<(IpAddr, IpMask, InterfaceId, PhysicalAddr)>,
    ) -> Self {
        Self {
            interfaces,
            routing_table,
        }
    }

    pub fn start(self) -> std::thread::JoinHandle<()> {
        std::thread::spawn(move || {
            let router = self;
            loop {
                for link in &router.interfaces {
                    if let Ok(Some(packet)) = link.try_recv() {
                        if let Some((interface, addr)) = router.route(&packet.dest) {
                            if let Err(SendError(packet)) =
                                router.interfaces[interface].send(packet, addr)
                            {
                                eprintln!(
                                    "Router send error for packet: [origin: {} dest: {}]",
                                    packet.origin, packet.dest
                                )
                            }
                        } else {
                            eprintln!(
                                "Packet can't be redirected: [origin: {} dest: {}]",
                                &packet.origin, &packet.dest
                            )
                        }
                    }
                }
            }
        })
    }

    fn route(&self, addr: &IpAddr) -> Option<(InterfaceId, PhysicalAddr)> {
        self.routing_table
            .iter()
            .find(|(dest, mask, _, _)| addr & mask == dest & mask)
            .map(|(_, _, i, a)| (*i, *a))
    }
}
