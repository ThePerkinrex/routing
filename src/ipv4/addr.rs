use std::{
    fmt::{Debug, Display},
    ops::BitAnd,
};

use crate::route::AddrMask;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IpV4Addr {
    addr: [u8; 4],
}

impl IpV4Addr {
    pub const fn new(addr: [u8; 4]) -> Self {
        Self { addr }
    }

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

pub const BROADCAST: IpV4Addr = IpV4Addr::new([255, 255, 255, 255]);
pub const DEFAULT: IpV4Addr = IpV4Addr::new([0, 0, 0, 0]);
pub const LOOPBACK: IpV4Addr = IpV4Addr::new([127, 0, 0, 1]); // Virtual

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IpV4Mask(u8);

impl IpV4Mask {
    pub fn new(mask: u8) -> Self {
        Self(mask.min(32))
    }

    fn get_mask(&self) -> [u8; 4] {
        let mut mask: u32 = 0;
        for _ in 0..self.0 {
            mask = 1 + (mask << 1);
        }
        mask <<= 32 - self.0;
        mask.to_be_bytes()
    }
}

impl Display for IpV4Mask {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", IpV4Addr::new(self.get_mask()))
    }
}

impl BitAnd<IpV4Addr> for IpV4Mask {
    type Output = IpV4Addr;

    fn bitand(self, rhs: IpV4Addr) -> Self::Output {
        let mut res = rhs.addr;
        for (real, mask) in res.iter_mut().zip(self.get_mask().into_iter()) {
            *real &= mask;
        }
        IpV4Addr::new(res)
    }
}

impl AddrMask<IpV4Addr> for IpV4Mask {
    type Specifity = u8;

    fn specifity(&self) -> Self::Specifity {
        self.0
    }
}
