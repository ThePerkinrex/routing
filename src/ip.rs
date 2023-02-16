use std::{
    fmt::{Debug, Display},
    ops::BitAnd,
};

use crate::tcp_ip::Payload;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IPv4(u32);

impl IPv4 {
    pub const fn new(octets: [u8; 4]) -> Self {
        Self(u32::from_be_bytes(octets))
    }

    pub const fn from_raw(octets: u32) -> Self {
        Self(octets)
    }
}

impl BitAnd for IPv4 {
    type Output = Self;

    fn bitand(self, rhs: Self) -> Self::Output {
        Self(self.0 & rhs.0)
    }
}

impl BitAnd for &IPv4 {
    type Output = IPv4;

    fn bitand(self, rhs: Self) -> Self::Output {
        IPv4(self.0 & rhs.0)
    }
}

impl Display for IPv4 {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let octets = self.0.to_be_bytes();
        write!(f, "{}.{}.{}.{}", octets[0], octets[1], octets[2], octets[3])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Ip<P, Addr = IPv4>
where
    P: Payload<1480>,
{
    pub origin: Addr,
    pub dest: Addr,
    pub payload: P,
}

impl<P, Addr> Payload<1500> for Ip<P, Addr>
where
    P: Payload<1480>,
{
    fn as_bytes(&self) -> Vec<u8> {
        todo!()
    }
}
