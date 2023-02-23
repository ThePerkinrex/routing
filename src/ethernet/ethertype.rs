#[repr(transparent)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct EtherType(u16);

impl EtherType {
    pub const fn from_u16(b: u16) -> Self {
        Self(b)
    }

    pub const fn to_u16(self) -> u16 {
        self.0
    }

    pub const IP_V4: Self = Self(0x0800);
    pub const IP_V6: Self = Self(0x86DD);
    pub const ARP: Self = Self(0x0806);
}
