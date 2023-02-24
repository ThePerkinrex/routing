use std::sync::Arc;

use tokio::sync::RwLock;

use crate::{chassis::LinkLayerId, route::RoutingTable};

use super::addr::{IpV4Addr, IpV4Mask};

pub struct IpV4ConfigInner {
    pub addr: IpV4Addr,
    pub routing: RoutingTable<IpV4Addr, IpV4Mask, LinkLayerId>,
    pub dhcp_run: bool,
}

pub type IpV4Config = Arc<RwLock<IpV4ConfigInner>>;
