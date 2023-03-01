use std::{collections::HashMap, io::Write, ops::DerefMut};

use clap::Parser;
use tokio::sync::RwLock;
use tracing::{error, info, warn};

use routing::{
    arp::ArpProcess,
    chassis::{self, Chassis, LinkLayerId, NicHandle, ProcessMessage, TransportLayerId},
    ethernet::nic::Nic,
    ipv4::{
        self,
        addr::{IpV4Addr, IpV4Mask},
        config::IpV4Config,
        IpV4Process,
    },
    mac::{self, Mac},
    route::RoutingEntry,
};

#[derive(Debug, clap::Parser)]
#[command(name = ">")]
enum GeneralCommands {
    Stop,
    New { name: String },
    List,
    Use { name: String },
}

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

mod arguments;

#[tokio::main]
async fn main() {
    start().await
}
async fn start() {
    tracing_subscriber::fmt::init();
    info!("Started process");
    let mut buffer = String::new();
    let stdin = std::io::stdin(); // We get `Stdin` here.
    let mut current_chassis = None;
    let mut chassis = HashMap::new();
    loop {
        buffer.clear();
        match current_chassis.as_ref() {
            None => {
                print!(" > ");
                std::io::stdout().flush().unwrap();
                stdin.read_line(&mut buffer).unwrap();
                match GeneralCommands::try_parse_from(
                    std::iter::once(">").chain(arguments::ArgumentsIter::new(buffer.trim())),
                ) {
                    Ok(GeneralCommands::Stop) => {
                        info!("stopping");
                        break;
                    }
                    #[allow(clippy::map_entry)]
                    Ok(GeneralCommands::New { name }) => {
                        if chassis.contains_key(&name) {
                            warn!("Chassis with name `{name}` already exists");
                            info!("Using chassis `{name}`");
                            current_chassis = Some(name);
                        } else {
                            info!("Created new chassis with name: {name}");
                            current_chassis = Some(name.clone());
                            let conf = IpV4Config::default();
                            let mut c = Chassis::new();
                            let (arp, arphandle) = ArpProcess::new(Some(conf.clone()), None);
                            c.add_network_layer_process(chassis::NetworkLayerId::Arp, arp);
                            let ip = IpV4Process::new(
                                conf.clone(),
                                arphandle.get_new_ipv4_handle().await.unwrap(),
                            );
                            c.add_network_layer_process(chassis::NetworkLayerId::Ipv4, ip);
                            chassis.insert(
                                name,
                                RwLock::new((
                                    c,
                                    conf,
                                    HashMap::<LinkLayerId, NicHandle>::default(),
                                    arphandle,
                                )),
                            );
                        }
                    }
                    Ok(GeneralCommands::Use { name }) => {
                        if chassis.contains_key(&name) {
                            info!("Using chassis `{name}`");
                            current_chassis = Some(name);
                        } else {
                            warn!("Chassis with name {name} doesn't exist")
                        }
                    }
                    Ok(GeneralCommands::List) => {
                        info!("Chassis list:");
                        for (i, chassis) in chassis.keys().enumerate() {
                            info!("   [{i}] {chassis}")
                        }
                    }
                    Err(e) => error!("{e}"),
                }
            }
            Some(name) => {
                print!("({name}) > ");
                let mut guard = chassis.get(name).unwrap().write().await;
                let (chassis_struct, ipv4_config, link_level_handles, arphandle) =
                    guard.deref_mut();
                std::io::stdout().flush().unwrap();
                stdin.read_line(&mut buffer).unwrap();
                match ChassisCommands::try_parse_from(
                    std::iter::once(">").chain(arguments::ArgumentsIter::new(buffer.trim())),
                ) {
                    Ok(ChassisCommands::Exit) => {
                        info!("Exiting chassis `{name}`");
                        current_chassis = None
                    }
                    Ok(ChassisCommands::Arp(cmd)) => match cmd {
                        ArpCmd::IpV4List => {
                            if let Some(data) = arphandle.get_ipv4_table().await {
                                let mut table =
                                    prettytable::table!(["IPv4", "MAC", "iface", "query time"]);
                                if data.is_empty() {
                                    table.add_empty_row();
                                }
                                for (ip, (mac, iface, t)) in data.into_iter() {
                                    table.add_row(prettytable::row![
                                        ip,
                                        mac,
                                        iface,
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
                            for (iface, handle) in link_level_handles.iter() {
                                info!("{iface:<5} {}", handle);
                            }
                        }
                        LinkCmd::Add {
                            link_type: LinkType::Eth,
                            id,
                            mac,
                        } => {
                            link_level_handles.insert(
                                LinkLayerId::Ethernet(id, mac),
                                chassis_struct.add_nic_with_id(id, Nic::new_with_mac(mac)),
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
                                let other_handles = &mut guard.2;
                                if let Some(handle) = link_level_handles.get_mut(&self_id) {
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
                                    ipv4_config.read().await.routing.print()
                                )
                            }
                            RouteCmd::Add {
                                destination,
                                mask,
                                next_hop,
                            } => todo!(),
                        },
                        IpChassisCommands::Set { addr } => {
                            info!("Setting chassis' {name} IPv4 addr to {addr}");
                            ipv4_config.write().await.addr = addr;
                        }
                        IpChassisCommands::Get => {
                            info!(
                                "Chassis {name} IPv4 addr is {}",
                                ipv4_config.read().await.addr
                            );
                        }
                    },

                    Err(e) => error!("{e}"),
                }
            }
        }
    }

    // let mut authority = mac::authority::SequentialAuthority::new([0, 0x69, 0x69]);
    // let nic_a = Nic::new(&mut authority);
    // let mut nic_b = Nic::new(&mut authority);
    // // nic_b.connect(&mut nic_a);
    // let mut chassis_a = Chassis::new();
    // nic_b = chassis_a
    //     .add_nic_with_id(0, nic_a)
    //     .connect(nic_b)
    //     .await
    //     .unwrap();
    // let mut chassis_b = Chassis::new();
    // chassis_b.add_nic_with_id(1, nic_b);
    // let ipv4_config = IpV4Config::default();
    // ipv4_config.write().await.addr = IpV4Addr::new([192, 168, 0, 30]);
    // info!(
    //     "Chassis B routing table:\n{}",
    //     ipv4_config.read().await.routing.print()
    // );
    // let mut arp_b = ArpProcess::new(Some(ipv4_config.clone()), None);
    // chassis_b.add_network_layer_process(
    //     chassis::NetworkLayerId::Ipv4,
    //     IpV4Process::new(ipv4_config, arp_b.new_ipv4_handle()),
    // );
    // chassis_b.add_network_layer_process(chassis::NetworkLayerId::Arp, arp_b);
    // let ipv4_config = IpV4Config::default();
    // ipv4_config.write().await.addr = IpV4Addr::new([192, 168, 0, 31]);
    // ipv4_config
    //     .write()
    //     .await
    //     .routing
    //     .add_route(RoutingEntry::new(
    //         ipv4::addr::DEFAULT,
    //         IpV4Addr::new([192, 168, 0, 30]),
    //         IpV4Mask::new(0),
    //         LinkLayerId::Ethernet(0, mac::BROADCAST),
    //     ));
    // info!(
    //     "Chassis A routing table:\n{}",
    //     ipv4_config.read().await.routing.print()
    // );
    // let mut arp_p = ArpProcess::new(Some(ipv4_config.clone()), None);
    // let handle = arp_p.new_ipv4_handle();
    // let tx = chassis_a.add_network_layer_process(
    //     chassis::NetworkLayerId::Ipv4,
    //     IpV4Process::new(ipv4_config, handle),
    // );
    // chassis_a.add_network_layer_process(chassis::NetworkLayerId::Arp, arp_p);
    // tx.send(ProcessMessage::Message(
    //     TransportLayerId::Tcp,
    //     chassis::NetworkTransportMessage::IPv4(IpV4Addr::new([192, 168, 0, 30]), vec![0x69]),
    // ))
    // .unwrap();
    // // debug!(
    // //     "Haddr: {:#?}",
    // //     handle
    // //         .get_haddr_timeout(
    // //             IpV4Addr::new([192, 168, 0, 30]),
    // //             std::time::Duration::from_millis(100)
    // //         )
    // //         .await
    // // );
    // tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    info!("Stopped process");
}