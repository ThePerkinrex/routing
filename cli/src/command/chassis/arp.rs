use tracing::info;

use crate::{chassis::ChassisData, ctrlc::CtrlC};

use super::ParsedChassisCommandRead;

#[derive(Debug, clap::Parser)]
pub enum Arp {
    IpV4List,
}

pub struct ArpCommand;

#[async_trait::async_trait]
impl ParsedChassisCommandRead<Arp> for ArpCommand {
    async fn run(
        &mut self,
        cmd: Arp,
        _: &CtrlC,
        _: String,
        ChassisData {
            ip_v4_arp_handle, ..
        }: &ChassisData,
    ) -> bool {
        match cmd {
            Arp::IpV4List => {
                if let Some(data) = ip_v4_arp_handle.get_ipv4_table().await {
                    let mut table = prettytable::table!(["IPv4", "interface", "MAC", "query time"]);
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
        }
        false
    }
}
