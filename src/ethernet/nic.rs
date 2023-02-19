use crate::{
    duplex_conn::DuplexBarrage,
    mac::{authority::MacAdminAuthority, Mac},
};

use super::{packet::EthernetPacket};

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

    pub const fn is_up(&self) -> bool {
        self.conn.is_some()
    }

    pub fn connect(&mut self, other: &mut Self) {
        if let Some(conn) = other.conn.as_ref() {
            self.conn = Some(conn.clone())
        }else{
            let duplex = DuplexBarrage::unbounded();
            self.conn = Some(duplex.clone());
            other.conn = Some(duplex);
        }
    }

    pub fn disconnect(&mut self) {
        self.conn = None
    }
}
