use super::addr::IpV4Addr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipv4Packet {
    // It has more header options
    destination: IpV4Addr,
    source: IpV4Addr,
    payload: Vec<u8>,
}
