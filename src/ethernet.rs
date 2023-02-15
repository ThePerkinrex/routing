use crate::tcp_ip::Payload;
use barrage::{Disconnected, SendError};

pub type PhysicalAddr = [u8; 6];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ethernet<P>
where
    P: Payload<1500>,
{
    origin: PhysicalAddr,
    dest: PhysicalAddr,
    payload: P,
}

impl<P> Payload<1524> for Ethernet<P>
where
    P: Payload<1500>,
{
    fn as_bytes(&self) -> Vec<u8> {
        todo!()
    }
}

#[derive(Clone)]
pub struct EthernetNet<P: Payload<1500> + Clone + Unpin> {
    tx: barrage::Sender<Ethernet<P>>,
    rx: barrage::Receiver<Ethernet<P>>,
}

impl<P: Payload<1500> + Clone + Unpin> EthernetNet<P> {
    pub fn new() -> Self {
        let (tx, rx) = barrage::unbounded();
        Self { rx, tx }
    }

    pub fn get_link(&self, addr: PhysicalAddr) -> EthernetLink<P> {
        EthernetLink {
            net: self.clone(),
            addr,
        }
    }
}

pub struct EthernetLink<P: Payload<1500> + Clone + Unpin> {
    net: EthernetNet<P>,
    addr: PhysicalAddr,
}

impl<P: Payload<1500> + Clone + Unpin> EthernetLink<P> {
    // pub fn recv(&self) -> Result<P, Disconnected> {
    //     loop {
    //         let packet = self.net.rx.recv()?;
    //         if packet.dest == self.addr {
    //             return Ok(packet.payload);
    //         }
    //     }
    // }

    pub fn try_recv(&self) -> Result<Option<P>, Disconnected> {
        loop {
            let packet = self.net.rx.try_recv()?;
            if let Some(packet) = packet {
                if packet.dest == self.addr {
                    return Ok(Some(packet.payload));
                }
            } else {
                return Ok(None);
            }
        }
    }

    pub fn send(&self, packet: P, dest: PhysicalAddr) -> Result<(), barrage::SendError<P>> {
        let packet = Ethernet {
            origin: self.addr,
            dest,
            payload: packet,
        };
        self.net
            .tx
            .send(packet)
            .map_err(|SendError(err)| SendError(err.payload))
    }
}
