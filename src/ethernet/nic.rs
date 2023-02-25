use barrage::{Receiver, Sender};

use crate::{
    duplex_conn::DuplexBarrage,
    mac::{authority::MacAdminAuthority, Mac},
};

use super::packet::EthernetPacket;

pub struct Nic {
    conn: Option<DuplexBarrage<EthernetPacket>>,
    addr: Mac,
}

impl Nic {
    pub const fn new_with_mac(addr: Mac) -> Self {
        Self { conn: None, addr }
    }

    pub fn new<A: MacAdminAuthority>(authority: &mut A) -> Self {
        Self::new_with_mac(authority.get_addr())
    }

    pub const fn mac(&self) -> Mac {
        self.addr
    }

    pub const fn is_up(&self) -> bool {
        self.conn.is_some()
    }

    pub fn connect(&mut self, other: &mut Self) {
        if let Some(conn) = other.conn.as_ref() {
            self.conn = Some(conn.clone())
        } else {
            let duplex = DuplexBarrage::unbounded();
            self.conn = Some(duplex.clone());
            other.conn = Some(duplex);
        }
    }

    pub fn disconnect(&mut self) {
        self.conn = None
    }

    pub async fn send(&self, p: EthernetPacket) -> Result<(), barrage::SendError<EthernetPacket>> {
        if let Some(duplex) = &self.conn {
            duplex.tx.send_async(p).await
        } else {
            Ok(())
        }
    }

    pub async fn recv(&self) -> Result<EthernetPacket, barrage::Disconnected> {
        if let Some(duplex) = &self.conn {
            duplex.rx.recv_async().await
        } else {
            Err(barrage::Disconnected)
        }
    }

    pub fn split(
        self,
    ) -> (
        Option<(Sender<EthernetPacket>, Receiver<EthernetPacket>)>,
        Mac,
    ) {
        (
            self.conn.map(|DuplexBarrage { rx, tx }| (tx, rx)),
            self.addr,
        )
    }

    pub fn join(conn: Option<(Sender<EthernetPacket>, Receiver<EthernetPacket>)>,
    mac: Mac,) -> Self {
        Self {
            conn: conn.map(|(tx, rx)| DuplexBarrage { tx, rx }),
            addr: mac
        }
    }
}
