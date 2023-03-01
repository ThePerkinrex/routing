use tracing::warn;

/// Represents type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IcmpPacket {
    /// 0
    EchoReply,

    // /// 3
    // DestinationUnreachable(u8), // TODO
    // /// 4
    // SourceQuench,

    // /// 5
    // RedirectMessage(u8), // TODO
    /// 8
    EchoRequest,
}

impl IcmpPacket {
    pub fn from_vec(data: &[u8]) -> Option<Self> {
        if data.len() < 4 {
            return None;
        }
        let typ = data[0];
        let code = data[1];
        let checksum = u16::from_be_bytes(data[2..4].try_into().unwrap()); // FIXME USE THIS
        match typ {
            0 => Some(Self::EchoReply),
            8 => Some(Self::EchoRequest),
            x => {
                warn!("Unknown ICMP type: {x}");
                None
            }
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let (typ, code, extra) = match self {
            Self::EchoReply => (0, 0, vec![]),
            Self::EchoRequest => (8, 0, vec![]),
        };
        let mut res = Vec::with_capacity(4 + extra.len());
        res.push(typ);
        res.push(code);
        res.extend_from_slice(&0u16.to_be_bytes()); // TODO Checksum
        res.extend_from_slice(&extra);
        res
    }
}
