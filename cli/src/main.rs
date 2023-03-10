use std::sync::Arc;

use tokio::sync::RwLock;
use tracing::{info, warn};

use routing::{mac::Mac, network::ipv4::addr::IpV4Addr};

use crate::{
    arguments::ArgumentsIter,
    chassis::ChassisManager,
    command::{
        general::{List, NewCommand, Stop, UseCommand},
        CommandManager, PCmd,
    },
    ctrlc::CtrlC,
    ping::PingCommand,
    traceroute::TracerouteCommand,
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
pub enum LinkType {
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
    let chassis = Arc::new(RwLock::new(ChassisManager::new()));
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

    let mut chassis_command_manager = CommandManager::<String, Result<bool, clap::Error>>::new();
    chassis_command_manager.register::<PCmd<_, _, _, _>, _, _>("source", static_source_command);
    chassis_command_manager.register::<PCmd<_, _, _, _>, _, _>("exit", command::chassis::Exit);
    chassis_command_manager.register::<PCmd<_, _, _, _>, _, _>("ping", PingCommand);
    chassis_command_manager.register::<PCmd<_, _, _, _>, _, _>("traceroute", TracerouteCommand);
    chassis_command_manager
        .register::<PCmd<_, _, _, _>, _, _>("arp", command::chassis::arp::ArpCommand);
    chassis_command_manager
        .register::<PCmd<_, _, _, _>, _, _>("link", command::chassis::link::LinkCommand);
    chassis_command_manager
        .register::<PCmd<_, _, _, _>, _, _>("ip-v4", command::chassis::ip_v4::IpV4Command);
    // register_commands(&mut chassis_command_manager);

    loop {
        (match current_chassis.as_ref() {
            None => {
                let buffer = commands.next().await.unwrap();
                info!("> {}", &*buffer);
                let args = ArgumentsIter::new(buffer.trim()).collect::<Vec<_>>();
                match general_command_manager
                    .call(&args, chassis.clone(), &ctrlc, ())
                    .await
                {
                    Some(Ok(x)) => Some(x),
                    Some(Err(e)) => {
                        warn!("Arguments error:\n{e}");
                        None
                    }
                    None => None,
                }
                
            }
            Some(name) => {
                // let ChassisData {
                //     c,
                //     ip_v4_conf,
                //     nics,
                //     ip_v4_arp_handle,
                //     icmp,
                //     udp_handles,
                //     processes,
                // } = guard.deref_mut();
                let buffer = commands.next().await.unwrap();
                info!("> {}", &*buffer);
                let args = ArgumentsIter::new(buffer.trim()).collect::<Vec<_>>();
                match chassis_command_manager
                    .call(&args, chassis.clone(), &ctrlc, name.clone())
                    .await
                {
                    Some(Ok(true)) => Some(None),
                    Some(Ok(false)) => None,
                    Some(Err(e)) => {
                        warn!("Arguments error:\n{e}");
                        None
                    }
                    None => None,
                }
            }
        })
        .map_or((), |x| current_chassis = x)
    }
}
