use std::collections::HashMap;

use routing::{
    chassis::{Chassis, LinkLayerId, NicHandle},
    network::{
        arp::GenericArpHandle,
        ipv4::{addr::IpV4Addr, config::IpV4Config},
    },
    process::ProcessManager,
    transport::{icmp::IcmpApi, udp::UdpHandleGeneric},
};
use tokio::sync::RwLock;

pub struct ChassisData {
    pub c: Chassis,
    pub ip_v4_conf: IpV4Config,
    pub nics: HashMap<LinkLayerId, NicHandle>,
    pub ip_v4_arp_handle: GenericArpHandle,
    pub icmp: IcmpApi,
    pub udp_handles: (UdpHandleGeneric<IpV4Addr>,),
    pub processes: ProcessManager,
}

impl ChassisData {
    pub fn new(
        c: Chassis,
        ip_v4_conf: IpV4Config,
        ip_v4_arp_handle: GenericArpHandle,
        icmp: IcmpApi,
        ip_v4_udp_handle: UdpHandleGeneric<IpV4Addr>,
    ) -> Self {
        Self {
            c,
            ip_v4_conf,
            nics: Default::default(),
            ip_v4_arp_handle,
            icmp,
            udp_handles: (ip_v4_udp_handle,),
            processes: Default::default(),
        }
    }
}

pub type ChassisManager = HashMap<String, RwLock<ChassisData>>;
