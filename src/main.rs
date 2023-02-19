use tracing::{debug, info};

use crate::mac::authority::MacAdminAuthority;

mod duplex_conn;
mod ethernet;
mod mac;

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    info!("Started process");
    let mut authority = mac::authority::SequentialAuthority::new([0, 0x69, 0x69]);
    let addr = authority.get_addr();
    debug!("Addr: {addr}");
    let addr = authority.get_addr();
    debug!("Addr: {addr}");
    info!("Stopped process");
}
