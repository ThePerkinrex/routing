#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Tag {
    // 3 bits
    priority_code_point: u8,
    drop_elegible: bool,
    // 12 bits
    vlan_id: u16,
}

impl Tag {
    pub const fn new(priority_code_point: u8, drop_elegible: bool, vlan_id: u16) -> Self {
        Self {
            priority_code_point,
            drop_elegible,
            vlan_id,
        }
    }

    pub fn to_vec(&self) -> Vec<u8> {
        let res = ((self.priority_code_point as u16) << 13)
            | (self.drop_elegible as u16) << 12
            | self.vlan_id;
        res.to_be_bytes().to_vec()
    }

    pub fn from_vec(data: &[u8]) -> Option<Option<Self>> {
        if data.len() < 2 {
            return None;
        }
        let tpid = u16::from_be_bytes(data[0..2].try_into().ok()?);

        Some(if tpid == 0x8100 {
            if data.len() < 4 {
                return None;
            }
            let tci = u16::from_be_bytes(data[0..2].try_into().ok()?);
            Some(Self {
                priority_code_point: (tci >> 13) as u8,
                drop_elegible: (tci & (1 << 12)) != 0,
                vlan_id: tci & 0x0fff,
            })
        } else {
            None
        })
    }

    pub const fn vlan_id(&self) -> u16 {
        self.vlan_id
    }
}
