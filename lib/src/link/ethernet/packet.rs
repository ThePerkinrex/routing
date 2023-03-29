use crate::mac::Mac;

use super::{dot1q, ethertype::EtherType};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthernetPacket {
    destination: Mac,
    source: Mac,
    // 16bit
    dot1q_tag: Option<dot1q::Tag>,
    ether_type: EtherType,
    pub payload: Vec<u8>,
}

impl EthernetPacket {
    pub fn new_generic(destination: Mac, source: Mac, payload: Vec<u8>) -> Option<Self> {
        if payload.len() <= 1500 {
            Some(Self {
                destination,
                source,
                dot1q_tag: None,
                ether_type: EtherType::from_u16(payload.len() as u16),
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
                dot1q_tag: None,
                ether_type: EtherType::IP_V4,
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
                dot1q_tag: None,
                ether_type: EtherType::IP_V6,
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
                dot1q_tag: None,
                ether_type: EtherType::ARP,
                payload,
            })
        } else {
            None
        }
    }

    pub fn set_dot1q(&mut self, dot1q_tag: dot1q::Tag) {
        self.dot1q_tag = Some(dot1q_tag)
    }

    pub fn remove_dot1q(&mut self) {
        self.dot1q_tag = None
    }

    pub const fn get_dot1q(&self) -> Option<dot1q::Tag> {
        self.dot1q_tag
    }

    pub const fn get_dest(&self) -> Mac {
        self.destination
    }

    pub const fn get_source(&self) -> Mac {
        self.source
    }

    pub const fn get_ether_type(&self) -> EtherType {
        self.ether_type
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut vec = Vec::with_capacity(14 + self.payload.len());
        vec.extend_from_slice(self.destination.as_slice());
        vec.extend_from_slice(self.source.as_slice());
        if let Some(dot1q_tag) = &self.dot1q_tag {
            vec.extend_from_slice(dot1q_tag.to_vec().as_slice());
        }
        vec.extend_from_slice(&self.ether_type.to_u16().to_be_bytes());
        vec.extend_from_slice(&self.payload);
        vec
    }

    pub fn from_vec(data: &[u8]) -> Option<Self> {
        if data.len() < 14 {
            return None;
        }
        let destination = Mac::new(data[0..6].try_into().ok()?);
        let source = Mac::new(data[6..12].try_into().ok()?);
        let mut offset = 12;
        let dot1q_tag = dot1q::Tag::from_vec(&data[offset..])?;
        if dot1q_tag.is_some() {
            offset += 4;
            if data.len() < 18 { // Baby jumbo
                return None
            }
        }
        let ether_type = EtherType::from_u16(u16::from_be_bytes(data[offset..(offset+2)].try_into().ok()?));
        Some(Self {
            destination,
            source,
            dot1q_tag,
            ether_type,
            payload: data[14..].to_vec(),
        })
    }
}
