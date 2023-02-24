use std::sync::Arc;

use tokio::sync::RwLock;

use crate::{chassis::LinkLayerId, route::RoutingTable};

use super::addr::{IpV4Addr, IpV4Mask, DEFAULT};

pub struct IpV4ConfigInner {
    pub addr: IpV4Addr,
    pub routing: RoutingTable<IpV4Addr, IpV4Mask, LinkLayerId>,
    pub dhcp_run: bool,
}

impl Default for IpV4ConfigInner {
    fn default() -> Self {
        Self {
            addr: DEFAULT,
            routing: Default::default(),
            dhcp_run: Default::default(),
        }
    }
}

pub type IpV4Config = Arc<RwLock<IpV4ConfigInner>>;
