use flume::{Sender, Receiver};
use tracing::{trace, debug, warn};

use std::collections::HashMap;

use crate::{ipv4::{addr::IpV4Addr}, chassis::{MidLevelProcess, NetworkLayerId, TransportLayerId, LinkLayerId, ProcessMessage}, mac::Mac, arp::packet::ArpPacket, ethernet::ethertype::EtherType};

pub mod packet;

type IpV6Addr = IpV4Addr;

#[derive(Debug, Clone)]
pub struct ArpProcess {
	ipv4: Option<IpV4Addr>,
	ipv6: Option<IpV6Addr>,
	ipv4_handle: Option<ArpHandle<Mac, IpV4Addr>>,
	ipv6_handle: Option<ArpHandle<Mac, IpV6Addr>>,

}

#[derive(Debug, Clone)]
pub struct ArpHandle<Addr, HAddr> {
	rx: Receiver<HAddr>,
	tx: Sender<Addr>
}

impl ArpProcess {
    pub const fn new(ipv4: Option<IpV4Addr>, ipv6: Option<IpV4Addr>) -> Self { Self { ipv4, ipv6, ipv4_handle: None, ipv6_handle: None } }
}

#[async_trait::async_trait]
impl MidLevelProcess<NetworkLayerId, TransportLayerId, LinkLayerId, (Mac, Vec<u8>), ()> for ArpProcess {
	async fn on_down_message(
        &mut self,
        (source_mac, msg): (Mac, Vec<u8>),
        down_id: LinkLayerId,
        down_sender: &HashMap<
            LinkLayerId,
            Sender<ProcessMessage<NetworkLayerId, LinkLayerId, (Mac, Vec<u8>)>>,
        >,
        up_sender: &HashMap<
            TransportLayerId,
            Sender<ProcessMessage<NetworkLayerId, TransportLayerId, ()>>,
        >,
    ) {
        trace!(ARP = ?self, "Recieved from {down_id} {source_mac}: {msg:?}");
        if let Some(arp_packet) = ArpPacket::from_vec(&msg) {
            match (arp_packet.htype, arp_packet.ptype) {
				(1, EtherType::IP_V4) => if let Some(ip) = self.ipv4 {
					if ip.as_slice() == arp_packet.target_protocol_address.as_slice() {
						trace!(ARP = ?self, "Received ARP IPv4 packet: {arp_packet:?}")
					}
				},
				(1, EtherType::IP_V6) => if let Some(ip) = self.ipv6 {
					if ip.as_slice() == arp_packet.target_protocol_address.as_slice() {
						trace!(ARP = ?self, "Received ARP IPv6 packet: {arp_packet:?}")
					}
				},
				(1, x) => warn!(ARP = ?self, "Unknown ptype: {x:?}"),
				(x, _) => warn!(ARP = ?self, "Unknown htype: {x}"),
			}
        }
    }
    async fn on_up_message(
        &mut self,
        msg: (),
        up_id: TransportLayerId,
        down_sender: &HashMap<
            LinkLayerId,
            Sender<ProcessMessage<NetworkLayerId, LinkLayerId, (Mac, Vec<u8>)>>,
        >,
        up_sender: &HashMap<
            TransportLayerId,
            Sender<ProcessMessage<NetworkLayerId, TransportLayerId, ()>>,
        >,
    ) {
        warn!(ARP = ?self, "Recieved packet from {up_id:?}");
        // for (id, sender) in down_sender {
        //     trace!(IP = ?self.ip, "Sending IPv4 packet to interface: {id}");
        //     sender
        //         .send_async(ProcessMessage::Message(
        //             NetworkLayerId::Ipv4,
        //             (
        //                 BROADCAST,
        //                 Ipv4Packet::new(ipv4::addr::BROADCAST, self.ip, vec![0x69, 0x69]).to_vec(),
        //             ),
        //         ))
        //         .await
        //         .unwrap();
        // }
    }
}