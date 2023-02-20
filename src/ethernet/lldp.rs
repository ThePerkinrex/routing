use bitflags::bitflags;

#[derive(Debug)]
pub struct TypeLengthValue {
    type_length: u16,
    value: Vec<u8>,
}

impl TypeLengthValue {
    pub fn new(t: TlvType, payload: Vec<u8>) -> Option<Self> {
        if payload.len() >= 512 {
            None
        } else {
            Some(Self {
                type_length: (t as u16) << 9 | (payload.len() as u16),
                value: payload,
            })
        }
    }

	pub fn to_vec(&self) -> Vec<u8> {
		let [a, b] = self.type_length.to_be_bytes();
		let mut res = Vec::with_capacity(2+self.value.len());
		res.push(a);
		res.push(b);
		res.extend_from_slice(&self.value);
		res
	}
}

#[derive(Debug)]
#[repr(u8)]
pub enum TlvType {
    End,
    ChassisId,
    PortId,
    TimeToLive,
    PortDescription,
    SystemName,
    SystemDescription,
    SystemCapabilities,
    ManagementAddress,
    Custom = 127,
}

pub struct LLDPPacket(pub(super) Vec<u8>);

pub fn build_lldp_packet(chassis_id: Vec<u8>, port_id: Vec<u8>, time_to_live: Vec<u8>, tlvs: &[TypeLengthValue]) -> LLDPPacket {
	let mut res = Vec::new();
	let chassis = TypeLengthValue::new(TlvType::ChassisId, chassis_id).unwrap();
	let port = TypeLengthValue::new(TlvType::PortId, port_id).unwrap();
	let ttl = TypeLengthValue::new(TlvType::TimeToLive, time_to_live).unwrap();
	let end = TypeLengthValue::new(TlvType::End, vec![]).unwrap();
	res.extend_from_slice(&chassis.to_vec());
	res.extend_from_slice(&port.to_vec());
	res.extend_from_slice(&ttl.to_vec());
	for tlv in tlvs {
		res.extend_from_slice(&tlv.to_vec());
	}
	res.extend_from_slice(&end.to_vec());
	LLDPPacket(res)
}

bitflags! {
	pub struct Capabilities: u8 {
		/// Bridge
		const B = 0b0000_0001;
		/// DOCSIS Cable Device 
		const C = 0b0000_0010;
		/// Other
		const O = 0b0000_0100;
		/// Repeater
		const P = 0b0000_1000;
		/// Router
		const R = 0b0001_0000;
		/// Station
		const S = 0b0010_0000;
		/// Telephone
		const T = 0b0100_0000;
		/// WLAN Access point
		const W = 0b1000_0000;
	}
}

pub fn capabilities_tlv(c: Capabilities) -> TypeLengthValue {
	TypeLengthValue::new(TlvType::SystemCapabilities, vec![c.bits()]).unwrap()
}
