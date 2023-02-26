use std::fmt::{Debug, Display};

pub mod authority;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Mac {
    addr: [u8; 6],
}

impl Mac {
    pub const fn new(addr: [u8; 6]) -> Self {
        Self { addr }
    }

    pub const fn is_universally_administered(&self) -> bool {
        (self.addr[0] & 2) == 0
    }

    pub const fn is_locally_administered(&self) -> bool {
        (self.addr[0] & 2) != 0
    }

    pub const fn is_unicast(&self) -> bool {
        (self.addr[0] & 1) == 0
    }

    pub const fn is_multicast(&self) -> bool {
        (self.addr[0] & 1) != 0
    }

    pub fn is_broadcast(&self) -> bool {
        self == &BROADCAST
    }

    pub fn manufacturer(&self) -> &[u8] {
        &self.addr[0..3]
    }

    pub const fn as_slice(&self) -> &[u8] {
        &self.addr
    }
}

impl Debug for Mac {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c, d, e, f] = self.addr;
        write!(fmt, "{a:02X}-{b:02X}-{c:02X}-{d:02X}-{e:02X}-{f:02X}")
    }
}

impl Display for Mac {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{self:?}")
    }
}

pub const BROADCAST: Mac = Mac::new([0xff, 0xff, 0xff, 0xff, 0xff, 0xff]);
// pub const LOCAL_LAN_LINK_MULTICAST: Mac = Mac::new([0x01, 0x80, 0xC2, 0x00, 0x00, 0x0E]);
