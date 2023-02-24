#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(transparent)]
pub struct ProtocolType(u8);

impl ProtocolType {
    pub const fn new(b: u8) -> Self {
        Self(b)
    }

    pub const fn inner(self) -> u8 {
        self.0
    }

    pub const ICMP: Self = Self::new(0x01);
    pub const TCP: Self = Self::new(0x06);
    pub const UDP: Self = Self::new(0x11);
}
