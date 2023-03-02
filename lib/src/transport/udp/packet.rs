pub struct UdpPacket {
    pub source_port: u16,
    pub destination_port: u16,
    pub payload: Vec<u8>,
}

impl UdpPacket {
    pub fn from_vec(data: &[u8]) -> Option<Self> {
        let source_port = u16::from_be_bytes(data[0..2].try_into().ok()?);
        let destination_port = u16::from_be_bytes(data[2..4].try_into().ok()?);
        let _length = u16::from_be_bytes(data[4..6].try_into().ok()?);
        let _checksum = u16::from_be_bytes(data[6..8].try_into().ok()?); // TODO
        let payload = data[8..].to_vec();
        Some(Self {
            source_port,
            destination_port,
            payload,
        })
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let mut res = Vec::with_capacity(8 + self.payload.len());
        res.extend_from_slice(&self.source_port.to_be_bytes());
        res.extend_from_slice(&self.destination_port.to_be_bytes());
        res.extend_from_slice(&(self.payload.len() as u16 + 8).to_be_bytes());
        res.extend_from_slice(&(0u16).to_be_bytes()); // TODO
        res
    }
}
