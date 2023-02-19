use super::Mac;

pub trait MacAdminAuthority {
    fn get_addr(&mut self) -> Mac;
}

#[derive(Debug, Clone)]
pub struct SequentialAuthority {
    oui: [u8; 3],
    curr: u32,
}

impl SequentialAuthority {
    pub const fn new(oui: [u8; 3]) -> Self {
        Self { oui, curr: 0 }
    }
}

impl MacAdminAuthority for SequentialAuthority {
    fn get_addr(&mut self) -> Mac {
        if self.curr > 0xffffff {
            panic!("Can't provide that many addresses");
        }
        let bytes = self.curr.to_be_bytes();
        self.curr += 1;
        Mac::new([
            self.oui[0],
            self.oui[1],
            self.oui[2],
            bytes[1],
            bytes[2],
            bytes[3],
        ])
    }
}
