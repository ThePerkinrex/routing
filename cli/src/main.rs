use std::{io::Write, collections::HashMap};

use clap::Parser;
use tracing::{info, error, warn};

use routing::{
    arp::ArpProcess,
    chassis::{self, Chassis, LinkLayerId, ProcessMessage, TransportLayerId},
    ethernet::nic::Nic,
    ipv4::{
        self,
        addr::{IpV4Addr, IpV4Mask},
        config::IpV4Config,
        IpV4Process,
    },
    mac::{self},
    route::RoutingEntry,
};

#[derive(Debug, clap::Parser)]
#[command(name=">")]
enum GeneralCommands {
    Stop,
    New {
        name: String
    },
    List,
    Use {
        name: String
    },
}

#[derive(Debug, clap::Parser)]
#[command(name=">")]
enum ChassisCommands {
    Exit,
    
    #[command(subcommand)]
    IpV4(IpChassisCommands)
}

#[derive(Debug, clap::Subcommand)]
enum IpChassisCommands {
    #[command(subcommand)]
    Route(RouteCmd),
    Set{
        addr: IpV4Addr
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
    }
}
mod arguments;

#[tokio::main]
async fn main() {
    start().await
}
async fn start(){
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
                match GeneralCommands::try_parse_from(std::iter::once(">").chain(arguments::ArgumentsIter::new(buffer.trim()))) {
                    Ok(GeneralCommands::Stop) => {info!("stopping"); break}
                    #[allow(clippy::map_entry)]
                    Ok(GeneralCommands::New { name }) => {
                        if chassis.contains_key(&name) {
                            warn!("Chassis with name `{name}` already exists");
                            info!("Using chassis `{name}`");
                            current_chassis = Some(name);
                        }else{
                            info!("Created new chassis with name: {name}");
                            current_chassis = Some(name.clone());
                            chassis.insert(name, (Chassis::new(), IpV4Config::default()));
                        }
                    }
                    Ok(GeneralCommands::Use { name }) => {
                        if chassis.contains_key(&name) {
                            info!("Using chassis `{name}`");
                            current_chassis = Some(name);
                        }else{
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
                let (chassis_struct, ipv4_config) = &chassis[name];
                std::io::stdout().flush().unwrap();
                stdin.read_line(&mut buffer).unwrap();
                match ChassisCommands::try_parse_from(std::iter::once(">").chain(arguments::ArgumentsIter::new(buffer.trim()))) {
                    Ok(ChassisCommands::Exit) => {info!("Exiting chassis `{name}`"); current_chassis = None}
                    Ok(ChassisCommands::IpV4(IpChassisCommands::Route(cmd))) => match cmd {
                        RouteCmd::List => {
                            info!("Chassis {name} IPv4 routes:\n{}", ipv4_config.read().await.routing.print())
                        },
                        RouteCmd::Add { destination, mask, next_hop } => todo!(),
                    }
                    Ok(ChassisCommands::IpV4(IpChassisCommands::Set{addr})) => {
                        info!("Setting chassis' {name} IPv4 addr to {addr}");
                        ipv4_config.write().await.addr = addr;
                    }
                    Ok(ChassisCommands::IpV4(IpChassisCommands::Get)) => {
                        info!("Chassis {name} IPv4 addr is {}", ipv4_config.read().await.addr);
                    }
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
