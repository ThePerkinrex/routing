use crate::ethernet::ethertype::EtherType;

#[repr(u16)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operation {
    Request = 1,
    Reply = 2,
}

impl Operation {
    const fn from_u16(b: u16) -> Option<Self> {
        match b {
            1 => Some(Self::Request),
            2 => Some(Self::Reply),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ArpPacket {
    pub(super) htype: u16, // 1 for Ethernet
    pub(super) ptype: EtherType,
    pub(super) hlen: u8, // Length of hardware address, MAC is 6,
    pub(super) plen: u8, // Length of protocol address, IPv4 is 4
    pub(super) operation: Operation,
    pub(super) sender_harware_address: Vec<u8>,
    pub(super) sender_protocol_address: Vec<u8>,
    pub(super) target_harware_address: Vec<u8>,
    pub(super) target_protocol_address: Vec<u8>,
}

const fn get_hardware_len(htype: u16) -> Option<u8> {
    match htype {
        1 => Some(6),
        x => None,
    }
}

const fn get_protocol_len(ptype: EtherType) -> Option<u8> {
    match ptype {
        EtherType::IP_V4 => Some(4),
        EtherType::IP_V6 => Some(16),
        x => None,
    }
}

impl ArpPacket {
    pub fn new_request(
        htype: u16,
        ptype: EtherType,
        sha: Vec<u8>,
        spa: Vec<u8>,
        tpa: Vec<u8>,
    ) -> Self {
        let hlen =
            get_hardware_len(htype).unwrap_or_else(|| unimplemented!("Unknown htype {htype:?}"));
        let plen =
            get_protocol_len(ptype).unwrap_or_else(|| unimplemented!("Unknown ptype {ptype:?}"));
        if sha.len() != hlen as usize {
            panic!("SHA size ({}) doesnt match expected ({hlen})", sha.len());
        }

        if spa.len() != plen as usize {
            panic!("SPA size ({}) doesnt match expected ({plen})", spa.len());
        }

        if tpa.len() != plen as usize {
            panic!("TPA size ({}) doesnt match expected ({plen})", tpa.len());
        }

        Self {
            htype,
            ptype,
            hlen,
            plen,
            operation: Operation::Request,
            sender_harware_address: sha,
            sender_protocol_address: spa,
            target_harware_address: vec![0; hlen as usize],
            target_protocol_address: tpa,
        }
    }

    pub fn new_reply(
        htype: u16,
        ptype: EtherType,
        sha: Vec<u8>,
        spa: Vec<u8>,
        tha: Vec<u8>,
        tpa: Vec<u8>,
    ) -> Self {
        let hlen =
            get_hardware_len(htype).unwrap_or_else(|| unimplemented!("Unknown htype {htype:?}"));
        let plen =
            get_protocol_len(ptype).unwrap_or_else(|| unimplemented!("Unknown ptype {ptype:?}"));
        if sha.len() != hlen as usize {
            panic!("SHA size ({}) doesnt match expected ({hlen})", sha.len());
        }

        if spa.len() != plen as usize {
            panic!("SPA size ({}) doesnt match expected ({plen})", spa.len());
        }

        if tha.len() != hlen as usize {
            panic!("THA size ({}) doesnt match expected ({hlen})", tha.len());
        }

        if tpa.len() != plen as usize {
            panic!("TPA size ({}) doesnt match expected ({plen})", tpa.len());
        }

        Self {
            htype,
            ptype,
            hlen,
            plen,
            operation: Operation::Reply,
            sender_harware_address: sha,
            sender_protocol_address: spa,
            target_harware_address: tha,
            target_protocol_address: tpa,
        }
    }

    pub fn from_vec(data: &[u8]) -> Option<Self> {
        if data.len() < 8 {
            return None;
        }
        let htype = u16::from_be_bytes(data[0..2].try_into().unwrap());
        let ptype = EtherType::from_u16(u16::from_be_bytes(data[2..4].try_into().unwrap()));
        let hlen = data[4];
        let plen = data[5];
        let operation = Operation::from_u16(u16::from_be_bytes(data[6..8].try_into().unwrap()))?;
        if data.len() != 8 + hlen as usize * 2 + plen as usize * 2 {
            return None;
        }
        let sha = data[8..(8 + hlen as usize)].to_vec();
        let spa = data[(8 + hlen as usize)..(8 + hlen as usize + plen as usize)].to_vec();
        let tha = data
            [(8 + hlen as usize + plen as usize)..(8 + (hlen as usize) * 2 + plen as usize)]
            .to_vec();
        let tpa = data[(8 + (hlen as usize) * 2 + plen as usize)
            ..(8 + (hlen as usize) * 2 + (plen as usize) * 2)]
            .to_vec();
        Some(Self {
            htype,
            ptype,
            hlen,
            plen,
            operation,
            sender_harware_address: sha,
            sender_protocol_address: spa,
            target_harware_address: tha,
            target_protocol_address: tpa,
        })
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut vec = Vec::with_capacity(8 + self.hlen as usize * 2 + self.plen as usize * 2);
        vec.extend_from_slice(&self.htype.to_be_bytes());
        vec.extend_from_slice(&self.ptype.to_u16().to_be_bytes());
        vec.extend_from_slice(&self.hlen.to_be_bytes());
        vec.extend_from_slice(&self.plen.to_be_bytes());
        vec.extend_from_slice(&(self.operation as u16).to_be_bytes());
        vec.extend_from_slice(&self.sender_harware_address);
        vec.extend_from_slice(&self.sender_protocol_address);
        vec.extend_from_slice(&self.target_harware_address);
        vec.extend_from_slice(&self.target_protocol_address);
        vec
    }
}
