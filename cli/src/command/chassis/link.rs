use std::sync::Arc;

use routing::{
    chassis::LinkLayerId,
    link::ethernet::nic::Nic,
    mac::{self, Mac},
};
use tokio::sync::RwLock;
use tracing::{info, warn};

use crate::{
    chassis::{ChassisData, ChassisManager},
    ctrlc::CtrlC,
    LinkType,
};

use super::ParsedChassisCommand;

#[derive(Debug, clap::Parser)]
pub enum Link {
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

pub struct LinkCommand;

#[async_trait::async_trait]
impl ParsedChassisCommand<Link> for LinkCommand {
    async fn run(
        &mut self,
        cmd: Link,
        chassis: Arc<RwLock<ChassisManager>>,
        _: &CtrlC,
        name: String,
    ) -> bool {
        let chassis_guard = chassis.read().await;
        let mut guard = chassis_guard.get(&name).unwrap().write().await;
        let ChassisData { c, nics, .. } = &mut *guard;
        match cmd {
            Link::List => {
                info!("Chassis {name} interfaces:");
                for (iface, handle) in nics.iter() {
                    info!("{iface:<5} {}", handle);
                }
            }
            Link::Add {
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
            Link::Connect {
                link_type: LinkType::Eth,
                id,
                other_chassis,
                other_id,
            } => {
                let self_id = LinkLayerId::Ethernet(id, mac::BROADCAST);
                let other_id = LinkLayerId::Ethernet(other_id, mac::BROADCAST);
                if let Some(guard) = chassis.read().await.get(&other_chassis) {
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
        }
        false
    }
}
