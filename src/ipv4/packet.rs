use tracing::{debug, warn};

use super::{addr::IpV4Addr, protocol::ProtocolType};

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum Ecn {
    NotECT = 0b00,
    ECT0 = 0b01,
    ECT1 = 0b10,
    CE = 0b11,
}

impl Ecn {
    const fn from_u8(b: u8) -> Option<Self> {
        match b {
            0b00 => Some(Self::NotECT),
            0b01 => Some(Self::ECT0),
            0b10 => Some(Self::ECT1),
            0b11 => Some(Self::CE),
            _ => None,
        }
    }
}

bitflags::bitflags! {
    /// 3 bit
    pub struct Flags: u8 {
        /// Dont fragment
        const DF = 0b010;
        /// More fragments
        const MF = 0b001;
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IpV4Header {
    // It has more header options
    // version: u8, // 4 bit
    /// Internet header length un 32 bit words (min 5). Includes options
    // ihl: u8, // 4 bit

    // Traffic class
    /// Differentiated Services Code Point
    dscp: u8, // 6 bit
    /// Explicit Congestion Notification
    ecn: Ecn, // 2 bit

    total_length: u16,

    identification: u16,

    flags: Flags,         // 3 bit
    fragment_offset: u16, // 13 bit,

    pub time_to_live: u8,

    protocol: ProtocolType,

    // Checksum (16 bit)
    // The checksum field is the 16 bit one's complement of the one's complement sum of all 16 bit words in the header.
    // For purposes of computing the checksum, the value of the checksum field is zero.

    // 32 bit each
    pub destination: IpV4Addr,
    pub source: IpV4Addr,

    // TODO decode
    options: Vec<u8>,
}

const fn ones_complement(mut num: u16) -> u16 {
    let mut cnt = 15;
    let mut flg = 0;

    // println!("Binary number: {:#0b}", num);

    while cnt >= 0 {
        if num & (1 << cnt) > 0 {
            flg = 1;
            num &= !(1 << cnt);
        } else if flg == 1 {
            num |= 1 << cnt;
        }

        cnt -= 1;
    }

    num
}

impl IpV4Header {
    pub fn new(
        // version: u8,
        // ihl: u8,
        dscp: u8,
        ecn: Ecn,
        payload_length: u16,
        identification: u16,
        flags: Flags,
        fragment_offset: u16,
        time_to_live: u8,
        protocol: ProtocolType,
        destination: IpV4Addr,
        source: IpV4Addr,
        options: Vec<u8>,
    ) -> Self {
        Self {
            dscp,
            ecn,
            total_length: payload_length + 20 + options.len() as u16,
            identification,
            flags,
            fragment_offset,
            time_to_live,
            protocol,
            destination,
            source,
            options,
        }
    }

    fn get_checksum(&self, extra: u16) -> u16 {
        let bytes = self.to_vec_checksum(extra);
        if bytes.len() % 2 != 0 {
            panic!("Erroneous ipv4 header");
        }
        let sum: u32 = bytes
            .chunks_exact(2)
            .map(|x| u16::from_be_bytes(<[u8; 2]>::try_from(x).unwrap()) as u32)
            .sum();
        let sum = ((sum & 0xffff) + ((sum & 0x000f_0000) >> 16)) as u16;

        ones_complement(sum)
    }

    pub fn from_vec(data: &[u8]) -> Option<(Self, usize)> {
        if data[0] >> 4 != 4 {
            warn!("IPv4 header version is not set correctly");
            return None;
        }
        let ihl = data[0] & 0x0f;
        if data.len() < ihl as usize * 4 {
            warn!("IPv4 header: Not enough data");
            return None;
        }
        let dscp = data[1] >> 2;
        let ecn = Ecn::from_u8(data[1] & 0b11)?;
        let total_length = u16::from_be_bytes(data[2..4].try_into().unwrap());
        if data.len() < total_length as usize {
            return None;
        }
        let identification = u16::from_be_bytes(data[4..6].try_into().unwrap());
        let fragment_and_flags = u16::from_be_bytes(data[6..8].try_into().unwrap());
        let fragment_offset = fragment_and_flags & 0x1fff;
        let flags = Flags::from_bits((fragment_and_flags >> 13) as u8)?;
        let time_to_live = data[8];
        let protocol = ProtocolType::new(data[9]);
        let checksum = u16::from_be_bytes(data[10..12].try_into().unwrap());
        let source = IpV4Addr::new(data[12..16].try_into().unwrap());
        let destination = IpV4Addr::new(data[16..20].try_into().unwrap());
        let options = data[20..(ihl as usize * 4)].to_vec();
        let res = Self {
            dscp,
            ecn,
            total_length,
            identification,
            flags,
            fragment_offset,
            time_to_live,
            protocol,
            destination,
            source,
            options,
        };
        let checksum = res.get_checksum(checksum);
        if checksum != 0 {
            warn!(header = ?res, "IPv4 header checksum error, calculation returned non zero ({})", checksum);
            return None;
        }

        Some((res, ihl as usize * 4))
    }

    fn to_vec_checksum(&self, checksum: u16) -> Vec<u8> {
        let mut vec = Vec::with_capacity(20 + self.options.len());
        let ihl = (((20 + self.options.len()) / 4) & 0x0f) as u8;
        vec.push(0x40 | ihl);
        vec.push(((self.dscp & 0b00111111) << 2) | ((self.ecn as u8) & 0b11));
        vec.extend_from_slice(&self.total_length.to_be_bytes());
        vec.extend_from_slice(&self.identification.to_be_bytes());
        vec.extend_from_slice(
            &((self.fragment_offset & 0x1fff) | (((self.flags.bits() & 0b111) as u16) << 13))
                .to_be_bytes(),
        );
        vec.push(self.time_to_live);
        vec.push(self.protocol.inner());
        vec.extend_from_slice(&checksum.to_be_bytes());
        vec.extend_from_slice(self.source.as_slice());
        vec.extend_from_slice(self.destination.as_slice());
        vec.extend_from_slice(&self.options);
        vec
    }

    fn to_vec(&self) -> Vec<u8> {
        self.to_vec_checksum(self.get_checksum(0))
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Ipv4Packet {
    pub header: IpV4Header,

    pub payload: Vec<u8>,
}

impl Ipv4Packet {
    pub fn new(header: IpV4Header, payload: Vec<u8>) -> Self {
        Self { header, payload }
    }

    pub fn from_vec(data: &[u8]) -> Option<Self> {
        let (header, left) = IpV4Header::from_vec(data)?;

        Some(Self::new(header, data[left..].to_vec()))
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut vec = self.header.to_vec();
        vec.extend_from_slice(&self.payload);
        vec
    }
}
