use either::Either;
use futures::FutureExt;
use tokio::task::JoinSet;
use tracing::{debug, info, trace, warn};

use crate::{
    chassis::{Chassis, LinkLayerId},
    ethernet::{nic::Nic, packet::EthernetPacket},
    mac::Mac,
};

mod chassis;
mod duplex_conn;
mod ethernet;
mod ipv4;
mod mac;

// TODO ARP

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!("Started process");
    let mut authority = mac::authority::SequentialAuthority::new([0, 0x69, 0x69]);
    let mut nic_a = Nic::new(&mut authority);
    let mut nic_b = Nic::new(&mut authority);
    nic_b.connect(&mut nic_a);
    let mut chassis_a = Chassis::new();
    chassis_a.add_nic_with_id(LinkLayerId::Ethernet(0), nic_a);
    let mut chassis_b = Chassis::new();
    chassis_b.add_nic_with_id(LinkLayerId::Ethernet(0), nic_b);
    chassis_a.add_network_layer_process(
        chassis::NetworkLayerId::Ipv4,
        move |down_link| async move {
            for (id, sender) in down_link.tx.iter() {
                trace!("Sending message through interface {id:?}");
                let _ = sender
                    .send_async(chassis::NetworkLinkMessage::Message(
                        chassis::NetworkLayerId::Ipv4,
                        mac::BROADCAST,
                        vec![69],
                    ))
                    .await;
            }
        },
    );
    tokio::time::sleep(std::time::Duration::from_secs(10)).await;
    info!("Stopped process");
}
