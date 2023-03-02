use tracing::warn;

/// Represents type
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum IcmpPacket {
    /// 0
    EchoReply { id: u16, seq: u16 },

    // /// 3
    // DestinationUnreachable(u8), // TODO
    // /// 4
    // SourceQuench,

    // /// 5
    // RedirectMessage(u8), // TODO
    /// 8
    EchoRequest { id: u16, seq: u16 },

    /// 11
    TimeExceeded(TimeExceeded),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TimeExceeded {
    /// 0 TTL exceeded in transit
    TtlTransit {
        // IP header and first 8 bytes
        data: Vec<u8>,
    },
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
            0 if data.len() == 8 => Some(Self::EchoReply {
                id: u16::from_be_bytes(data[4..6].try_into().ok()?),
                seq: u16::from_be_bytes(data[6..8].try_into().ok()?),
            }),
            8 if data.len() == 8 => Some(Self::EchoRequest {
                id: u16::from_be_bytes(data[4..6].try_into().ok()?),
                seq: u16::from_be_bytes(data[6..8].try_into().ok()?),
            }),
            11 => match code {
                0 => Some(Self::TimeExceeded(TimeExceeded::TtlTransit {
                    data: data[4..].to_vec(),
                })),
                x => {
                    warn!("Unknown ICMP time exceeded code: {x}");
                    None
                }
            },
            x => {
                warn!("Unknown ICMP type: {x}");
                None
            }
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let (typ, code, extra) = match self {
            Self::EchoReply { id, seq } => (0, 0, {
                let ([a, b], [c, d]) = (id.to_be_bytes(), seq.to_be_bytes());
                vec![a, b, c, d]
            }),
            Self::EchoRequest { id, seq } => (8, 0, {
                let ([a, b], [c, d]) = (id.to_be_bytes(), seq.to_be_bytes());
                vec![a, b, c, d]
            }),
            Self::TimeExceeded(t) => match t {
                TimeExceeded::TtlTransit { data } => (11, 0, data.clone()),
            },
        };
        let mut res = Vec::with_capacity(4 + extra.len());
        res.push(typ);
        res.push(code);
        res.extend_from_slice(&0u16.to_be_bytes()); // TODO Checksum
        res.extend_from_slice(&extra);
        res
    }
}
