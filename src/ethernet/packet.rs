use crate::mac::Mac;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthernetPacket {
    destination: Mac,
    source: Mac,
    ether_type: u16,
    payload: Vec<u8>,
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

    pub fn to_vec(&self) -> Vec<u8> {
        let mut vec = Vec::with_capacity(14 + self.payload.len());
        vec.extend_from_slice(self.destination.as_slice());
        vec.extend_from_slice(self.source.as_slice());
        vec.extend_from_slice(&self.ether_type.to_be_bytes());
        vec.extend_from_slice(&self.payload);
        vec
    }
}
