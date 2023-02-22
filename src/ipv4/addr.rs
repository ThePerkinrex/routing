use std::fmt::{Debug, Display};

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub struct IpV4Addr {
    addr: [u8; 4],
}

impl IpV4Addr {
    pub const fn new(addr: [u8; 4]) -> Self { Self { addr } }

    pub const fn as_arr(self) -> [u8; 4] {
        self.addr
    }

    pub const fn as_slice(&self) -> &[u8; 4] {
        &self.addr
    }
}

impl Debug for IpV4Addr {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let [a, b, c, d] = self.addr;
        write!(fmt, "{a}.{b}.{c}.{d}")
    }
}

impl Display for IpV4Addr {
    fn fmt(&self, fmt: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(fmt, "{self:?}")
    }
}

pub const BROADCAST: IpV4Addr = IpV4Addr::new([255,255,255,255]);
pub const DEFAULT: IpV4Addr = IpV4Addr::new([0,0,0,0]);
pub const LOOPBACK: IpV4Addr = IpV4Addr::new([127,0,0,1]); // Virtual