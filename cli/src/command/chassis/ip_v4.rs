use routing::{
    chassis::LinkLayerId,
    mac,
    network::ipv4::addr::{IpV4Addr, IpV4Mask},
    route::RoutingEntry,
};
use tracing::{info, warn};

use crate::{chassis::ChassisData, ctrlc::CtrlC, LinkType};

use super::ParsedChassisCommandRead;

#[derive(Debug, clap::Parser)]
pub enum IpV4 {
    #[command(subcommand)]
    Route(RouteCmd),
    Set {
        addr: IpV4Addr,
    },
    Get,
}

#[derive(Debug, clap::Subcommand)]
pub enum RouteCmd {
    List,
    Add {
        destination: IpV4Addr,
        mask: u8,
        next_hop: IpV4Addr,
        iface_type: LinkType,
        iface_id: u16,
    },
    Get {
        destination: IpV4Addr,
    },
}

pub struct IpV4Command;

#[async_trait::async_trait]
impl ParsedChassisCommandRead<IpV4> for IpV4Command {
    async fn run(
        &mut self,
        cmd: IpV4,
        _: &CtrlC,
        name: String,
        ChassisData { ip_v4_conf, .. }: &ChassisData,
    ) -> bool {
        match cmd {
            IpV4::Route(cmd) => match cmd {
                RouteCmd::List => {
                    info!(
                        "Chassis {name} IPv4 routes:\n{}",
                        ip_v4_conf.read().await.routing.print()
                    )
                }
                RouteCmd::Add {
                    destination,
                    mask,
                    next_hop,
                    iface_type,
                    iface_id,
                } => {
                    ip_v4_conf
                        .write()
                        .await
                        .routing
                        .add_route(RoutingEntry::new(
                            destination,
                            next_hop,
                            IpV4Mask::new(mask),
                            match iface_type {
                                LinkType::Eth => LinkLayerId::Ethernet(iface_id, mac::BROADCAST),
                            },
                        ));
                }
                RouteCmd::Get { destination } => ip_v4_conf
                    .read()
                    .await
                    .routing
                    .get_route(destination)
                    .map_or_else(
                        || {
                            warn!("Route to {destination} not found");
                        },
                        |(route, iface)| {
                            info!("Route to {destination} through {route} ({iface})");
                        },
                    ),
            },
            IpV4::Set { addr } => {
                info!("Setting chassis' {name} IPv4 addr to {addr}");
                ip_v4_conf.write().await.addr = addr;
            }
            IpV4::Get => {
                info!(
                    "Chassis {name} IPv4 addr is {}",
                    ip_v4_conf.read().await.addr
                );
            }
        }
        false
    }
}
