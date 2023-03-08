use routing::{
    chassis::{Chassis, NetworkLayerId, TransportLayerId},
    network::{
        arp::ArpProcess,
        ipv4::{config::IpV4Config, IpV4Process},
    },
    transport::{
        icmp::IcmpProcess,
        udp::{UdpProcess, UdpProcessGeneric},
    },
};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::chassis::{ChassisData, ChassisManager};

use super::ParsedCommand;

#[derive(Debug, clap::Parser)]
pub struct Stop;

#[async_trait::async_trait]
impl ParsedCommand<Self, (), Option<String>> for Stop {
    async fn run(&mut self, _: Self, _: &mut ChassisManager, (): ()) -> Option<String> {
        info!("Exiting");
        std::process::exit(0);
    }
}

#[derive(Debug, clap::Parser)]
pub struct List;

#[async_trait::async_trait]
impl ParsedCommand<Self, (), Option<String>> for List {
    async fn run(&mut self, _: Self, chassis: &mut ChassisManager, (): ()) -> Option<String> {
        info!("Chassis list:");
        for (i, chassis) in chassis.keys().enumerate() {
            info!("   [{i}] {chassis}")
        }
        None
    }
}

#[derive(Debug, clap::Parser)]
pub struct New {
    name: String,
}

pub struct NewCommand;

#[async_trait::async_trait]
impl ParsedCommand<New, (), Option<String>> for NewCommand {
    #[allow(clippy::map_entry)]
    async fn run(
        &mut self,
        New { name }: New,
        chassis: &mut ChassisManager,
        (): (),
    ) -> Option<String> {
        if chassis.contains_key(&name) {
            warn!("Chassis with name `{name}` already exists");
            info!("Using chassis `{name}`");
            Some(name)
        } else {
            info!("Created new chassis with name: {name}");
            let current_chassis = Some(name.clone());
            let conf = IpV4Config::default();
            let mut c = Chassis::new();
            let (arp, arphandle) = ArpProcess::new(Some(conf.clone()), None);
            c.add_network_layer_process(NetworkLayerId::Arp, arp);
            let ip = IpV4Process::new(conf.clone(), arphandle.get_new_ipv4_handle().await.unwrap());
            c.add_network_layer_process(NetworkLayerId::Ipv4, ip);
            let (icmp, icmp_api) = IcmpProcess::new();
            c.add_transport_layer_process(TransportLayerId::Icmp, icmp);
            let (udp_ip_v4, udp_ip_v4_handle) = UdpProcessGeneric::new();
            c.add_transport_layer_process(TransportLayerId::Udp, UdpProcess::new(udp_ip_v4));
            chassis.insert(
                name,
                RwLock::new(ChassisData::new(
                    c,
                    conf,
                    arphandle,
                    icmp_api,
                    udp_ip_v4_handle,
                )),
            );
            current_chassis
        }
    }
}

#[derive(Debug, clap::Parser)]
pub struct Use {
    name: String,
}

pub struct UseCommand;

#[async_trait::async_trait]
impl ParsedCommand<Use, (), Option<String>> for UseCommand {
    async fn run(
        &mut self,
        Use { name }: Use,
        chassis: &mut ChassisManager,
        (): (),
    ) -> Option<String> {
        if chassis.contains_key(&name) {
            info!("Using chassis `{name}`");
            Some(name)
        } else {
            warn!("Chassis with name {name} doesn't exist");
            None
        }
    }
}
