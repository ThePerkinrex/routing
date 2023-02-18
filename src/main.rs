use tracing::{debug, info};

fn main() {
    tracing_subscriber::fmt::init();
    info!("Started process");


    info!("Stopped process");
}
