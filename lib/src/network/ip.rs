use super::ipv4::addr::IpV4Addr;

pub trait Ip {}

impl Ip for IpV4Addr {}
