use std::{collections::HashMap, io::Write, ops::DerefMut, path::PathBuf};

use clap::Parser;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use routing::{
    chassis::{Chassis, LinkLayerId, NetworkLayerId, NicHandle, TransportLayerId},
    link::ethernet::nic::Nic,
    mac::{self, Mac},
    network::arp::ArpProcess,
    network::ipv4::{
        addr::{IpV4Addr, IpV4Mask},
        config::IpV4Config,
        IpV4Process,
    },
    route::RoutingEntry,
    transport::{
        icmp::IcmpProcess,
        udp::{UdpProcess, UdpProcessGeneric},
    },
};

use crate::{
    arguments::ArgumentsIter,
    chassis::{ChassisData, ChassisManager},
    command::{
        general::{List, NewCommand, Stop, UseCommand},
        CommandManager, PCmd, ParsedCommand,
    },
    ctrlc::CtrlC,
};

mod arguments;
mod chassis;
mod cli;
mod command;
mod ctrlc;
mod ping;
mod traceroute;
// #[derive(Debug, clap::Parser)]
// #[command(name = ">")]
// enum GeneralCommands {
//     Stop,
//     New { name: String },
//     List,
//     Use { name: String },
//     Source { path: PathBuf },
// }

#[derive(Debug, clap::Parser)]
#[command(name = ">")]
enum ChassisCommands {
    Exit,
    #[command(subcommand)]
    Link(LinkCmd),
    #[command(subcommand)]
    IpV4(IpChassisCommands),
    #[command(subcommand)]
    Arp(ArpCmd),
    Ping(ping::Ping),
    Traceroute(traceroute::Traceroute),
}

#[derive(Debug, clap::Subcommand)]
enum IpChassisCommands {
    #[command(subcommand)]
    Route(RouteCmd),
    Set {
        addr: IpV4Addr,
    },
    Get,
}

#[derive(Debug, clap::Subcommand)]
enum RouteCmd {
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

#[derive(Debug, clap::Subcommand)]
enum ArpCmd {
    IpV4List,
}

#[derive(Debug, clap::Subcommand)]
enum LinkCmd {
    List,
    Add {
        link_type: LinkType,
        id: u16,
        mac: Mac,
    },
    Connect {
        link_type: LinkType,
        id: u16,
        other_chassis: String,
        other_id: u16,
    },
}

#[derive(Debug, Clone, clap::ValueEnum)]
enum LinkType {
    Eth,
}

#[tokio::main]
async fn main() {
    start().await
}
async fn start() {
    tracing_subscriber::fmt::init();
    info!("Started process");
    // let mut buffer = String::new();
    // let stdin = std::io::stdin(); // We get `Stdin` here.
    let mut current_chassis: Option<String> = None;
    let mut chassis = ChassisManager::new();
    // let mut lines = Vec::new();
    let ctrlc = CtrlC::new().unwrap();
    let base_handler = ctrlc.add_handler().await;
    tokio::spawn(async move {
        base_handler.next().await;
        std::process::exit(1);
    });

    let (commands, source_command) = cli::cli();

    let static_source_command: &'static _ = &*Box::leak(Box::new(source_command));

    let mut general_command_manager =
        CommandManager::<(), Result<Option<String>, clap::Error>>::new();
    general_command_manager.register::<PCmd<_, _, _, _>, _, _>("stop", Stop);
    general_command_manager.register::<PCmd<_, _, _, _>, _, _>("list", List);
    general_command_manager.register::<PCmd<_, _, _, _>, _, _>("new", NewCommand);
    general_command_manager.register::<PCmd<_, _, _, _>, _, _>("use", UseCommand);
    general_command_manager.register::<PCmd<_, _, _, _>, _, _>("source", static_source_command);

    let mut chassis_command_manager =
        CommandManager::<ChassisData, Result<bool, clap::Error>>::new();
    chassis_command_manager.register::<PCmd<_, _, _, _>, _, _>("source", static_source_command);

    loop {
        match current_chassis.as_ref() {
            None => {
                let buffer = commands.next().await.unwrap();
                info!("Executing {}", &*buffer);
                let args = ArgumentsIter::new(buffer.trim()).collect::<Vec<_>>();
                match general_command_manager.call(&args, &mut chassis, ()).await {
                    Some(Ok(x)) => current_chassis = x,
                    Some(Err(e)) => warn!("Arguments error:\n{e}"),
                    None => (),
                }
            }
            Some(name) => {
                let mut guard = chassis.get(name).unwrap().write().await;
                let ChassisData {
                    c,
                    ip_v4_conf,
                    nics,
                    ip_v4_arp_handle,
                    icmp,
                    udp_handles,
                    processes,
                } = guard.deref_mut();
                let buffer = commands.next().await.unwrap();
                match ChassisCommands::try_parse_from(
                    std::iter::once(">").chain(arguments::ArgumentsIter::new(buffer.trim())),
                ) {
                    Ok(ChassisCommands::Exit) => {
                        info!("Exiting chassis `{name}`");
                        current_chassis = None
                    }
                    Ok(ChassisCommands::Ping(args)) => ping::ping(args, icmp, &ctrlc).await,
                    Ok(ChassisCommands::Traceroute(args)) => {
                        traceroute::traceroute(args, icmp, &ctrlc, &udp_handles).await
                    }
                    Ok(ChassisCommands::Arp(cmd)) => match cmd {
                        ArpCmd::IpV4List => {
                            if let Some(data) = ip_v4_arp_handle.get_ipv4_table().await {
                                let mut table =
                                    prettytable::table!(["IPv4", "interface", "MAC", "query time"]);
                                if data.is_empty() {
                                    table.add_empty_row();
                                }
                                for ((ip, iface), (mac, t)) in data.into_iter() {
                                    table.add_row(prettytable::row![
                                        ip,
                                        iface,
                                        mac,
                                        t.format("%d/%m/%Y %H:%M:%S%.f")
                                    ]);
                                }
                                info!("ARP IPv4 list:\n{table}");
                            }
                        }
                    },
                    Ok(ChassisCommands::Link(cmd)) => match cmd {
                        LinkCmd::List => {
                            info!("Chassis {name} interfaces:");
                            for (iface, handle) in nics.iter() {
                                info!("{iface:<5} {}", handle);
                            }
                        }
                        LinkCmd::Add {
                            link_type: LinkType::Eth,
                            id,
                            mac,
                        } => {
                            nics.insert(
                                LinkLayerId::Ethernet(id, mac),
                                c.add_nic_with_id(id, Nic::new_with_mac(mac)),
                            );
                            info!("NIC added");
                        }
                        LinkCmd::Connect {
                            link_type: LinkType::Eth,
                            id,
                            other_chassis,
                            other_id,
                        } => {
                            let self_id = LinkLayerId::Ethernet(id, mac::BROADCAST);
                            let other_id = LinkLayerId::Ethernet(other_id, mac::BROADCAST);
                            if let Some(guard) = chassis.get(&other_chassis) {
                                let mut guard = guard.write().await;
                                let other_handles = &mut guard.nics;
                                if let Some(handle) = nics.get_mut(&self_id) {
                                    if let Some(other_handle) = other_handles.get_mut(&other_id) {
                                        if handle.connect_other(other_handle).await {
                                            info!("Connected");
                                        } else {
                                            warn!("Didn't connect")
                                        }
                                    } else {
                                        warn!("Chassis `{other_chassis}` doesn't have interface {other_id}")
                                    }
                                } else {
                                    warn!("Chassis `{name}` doesn't have interface {self_id}")
                                }
                            } else {
                                warn!("Chassis `{other_chassis}` doesn't exist")
                            }
                        }
                    },
                    Ok(ChassisCommands::IpV4(cmd)) => match cmd {
                        IpChassisCommands::Route(cmd) => match cmd {
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
                                            LinkType::Eth => {
                                                LinkLayerId::Ethernet(iface_id, mac::BROADCAST)
                                            }
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
                        IpChassisCommands::Set { addr } => {
                            info!("Setting chassis' {name} IPv4 addr to {addr}");
                            ip_v4_conf.write().await.addr = addr;
                        }
                        IpChassisCommands::Get => {
                            info!(
                                "Chassis {name} IPv4 addr is {}",
                                ip_v4_conf.read().await.addr
                            );
                        }
                    },

                    Err(e) => error!("{e}"),
                }
            }
        }
    }
}
