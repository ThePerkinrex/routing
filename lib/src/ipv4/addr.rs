use std::{
    error::Error,
    fmt::{Debug, Display},
    num::ParseIntError,
    ops::BitAnd,
    str::FromStr,
};

use tracing::debug;

use crate::route::AddrMask;

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct IpV4Addr {
    addr: [u8; 4],
}

#[derive(Debug, Clone)]
pub enum IPv4ParseError {
    ParseByteError(ParseIntError),
    LengthError,
}

impl Display for IPv4ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{self:?}")
    }
}

impl Error for IPv4ParseError {}

impl FromStr for IpV4Addr {
    type Err = IPv4ParseError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let split = s
            .split('.')
            .map(u8::from_str)
            .collect::<Result<Vec<_>, _>>()
            .map_err(IPv4ParseError::ParseByteError)?;

        Ok(Self::new(
            split.try_into().map_err(|_| IPv4ParseError::LengthError)?,
        ))
    }
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
        // debug!("mask: {mask}");
        mask = mask.overflowing_shl(32 - self.0 as u32).0;
        mask.to_be_bytes()
    }
}

impl From<u8> for IpV4Mask {
    fn from(value: u8) -> Self {
        Self::new(value)
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
