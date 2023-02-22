use crate::mac::Mac;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthernetPacket {
    destination: Mac,
    source: Mac,
    ether_type: u16,
    pub payload: Vec<u8>,
}

impl EthernetPacket {
    pub fn new_generic(destination: Mac, source: Mac, payload: Vec<u8>) -> Option<Self> {
        if payload.len() <= 1500 {
            Some(Self {
                destination,
                source,
                ether_type: payload.len() as u16,
                payload,
            })
        } else {
            None
        }
    }

    pub fn new_ip_v4(destination: Mac, source: Mac, payload: Vec<u8>) -> Option<Self> {
        if payload.len() <= 1500 {
            Some(Self {
                destination,
                source,
                ether_type: 0x800,
                payload,
            })
        } else {
            None
        }
    }

    pub fn new_ip_v6(destination: Mac, source: Mac, payload: Vec<u8>) -> Option<Self> {
        if payload.len() <= 1500 {
            Some(Self {
                destination,
                source,
                ether_type: 0x86DD,
                payload,
            })
        } else {
            None
        }
    }

    pub fn new_arp(destination: Mac, source: Mac, payload: Vec<u8>) -> Option<Self> {
        if payload.len() <= 1500 {
            Some(Self {
                destination,
                source,
                ether_type: 0x0806,
                payload,
            })
        } else {
            None
        }
    }

    pub const fn get_dest(&self) -> Mac {
        self.destination
    }

    pub const fn get_ether_type(&self) -> u16 {
        self.ether_type
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut vec = Vec::with_capacity(14 + self.payload.len());
        vec.extend_from_slice(self.destination.as_slice());
        vec.extend_from_slice(self.source.as_slice());
        vec.extend_from_slice(&self.ether_type.to_be_bytes());
        vec.extend_from_slice(&self.payload);
        vec
    }

    pub fn from_vec(data: &[u8]) -> Option<Self> {
        if data.len() < 14 {
            return None
        }
        let destination = Mac::new(data[0..6].try_into().ok()?);
        let source = Mac::new(data[6..12].try_into().ok()?);
        let ether_type = u16::from_be_bytes(data[12..14].try_into().ok()?);
        Some(Self {
                    destination,
                    source,
                    ether_type,
                    payload: data[14..].to_vec(),
                })
    }
}
