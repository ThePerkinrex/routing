use super::{addr::IpV4Addr, protocol::ProtocolType};

#[repr(u8)]
#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ECN {
    NotECT = 0b00,
    ECT0 = 0b01,
    ECT1 = 0b10,
    CE = 0b11,
}

bitflags::bitflags! {
    /// 3 bit
    struct Flags: u8 {
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
    ecn: ECN, // 2 bit

    total_length: u16,

    identification: u16,

    flags: Flags,         // 3 bit
    fragment_offset: u16, // 13 bit,

    time_to_live: u8,

    protocol: ProtocolType,

    // Checksum (16 bit)
    // The checksum field is the 16 bit one's complement of the one's complement sum of all 16 bit words in the header.
    // For purposes of computing the checksum, the value of the checksum field is zero.

    // 32 bit each
    destination: IpV4Addr,
    source: IpV4Addr,

    // TODO decode
    options: Vec<u8>,
}

fn ones_complement(mut num: u16) -> u16 {
    let mut cnt = 15;
    let mut tmp = 0;
    let mut flg = 0;

    println!("Binary number: {:#0b}", num);

    while cnt >= 0 {
        tmp = num & (1 << cnt);
        if tmp > 0 {
            flg = 1;
            num &= !(1 << cnt);
        } else if flg == 1 {
            num |= 1 << cnt;
        }

        cnt = cnt - 1;
    }

    num
}

impl IpV4Header {
    fn new(
        // version: u8,
        // ihl: u8,
        dscp: u8,
        ecn: ECN,
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

    // pub fn from_vec(data: &[u8]) -> Option<Self> {
    //     if data.len() < 8 {
    //         return None;
    //     }

    //     let destination = IpV4Addr::new(data[0..4].try_into().ok()?);
    //     let source = IpV4Addr::new(data[4..8].try_into().ok()?);
    //     Some(Self {
    //         destination,
    //         source,
    //         payload: data[8..].to_vec(),
    //     })
    // }

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
    header: IpV4Header,

    payload: Vec<u8>,
}

impl Ipv4Packet {
    pub fn new(header: IpV4Header, payload: Vec<u8>) -> Self {
        Self { header, payload }
    }

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
