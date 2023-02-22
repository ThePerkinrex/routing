use super::addr::IpV4Addr;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipv4Packet {
    // It has more header options
    destination: IpV4Addr,
    source: IpV4Addr,
    payload: Vec<u8>,
}

impl Ipv4Packet {
    pub fn new(destination: IpV4Addr, source: IpV4Addr, payload: Vec<u8>) -> Self { Self { destination, source, payload } }

    pub fn from_vec(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }

        let destination = IpV4Addr::new(data[0..4].try_into().ok()?);
        let source = IpV4Addr::new(data[4..8].try_into().ok()?);
        Some(Self {
                    destination,
                    source,
                    payload: data[8..].to_vec(),
                })
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut vec = Vec::with_capacity(8 + self.payload.len());
        vec.extend_from_slice(self.destination.as_slice());
        vec.extend_from_slice(self.source.as_slice());
        vec.extend_from_slice(&self.payload);
        vec
    }
}
