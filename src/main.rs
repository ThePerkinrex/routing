use tracing::{debug, info};

use crate::{
    ethernet::{EthernetNet, PhysicalAddr},
    ip::IPv4,
    link::Link,
    router::Router,
};

mod ethernet;
mod ip;
mod link;
mod router;
mod tcp_ip;

fn main() {
    tracing_subscriber::fmt::init();
    let net1 = EthernetNet::new();
    let link_a = net1.get_link(PhysicalAddr::new([0, 0, 0, 0, 0, 0]));
    let link_b = net1.get_link(PhysicalAddr::new([0, 0, 0, 0, 0, 1]));
    let link_router_net1 = net1.get_link(PhysicalAddr::new([0, 0, 0, 0, 0, 2]));
    let net2 = EthernetNet::new();
    let link_router_net2 = net2.get_link(PhysicalAddr::new([0, 0, 0, 0, 0, 0]));
    let link_c = net2.get_link(PhysicalAddr::new([0, 0, 0, 0, 0, 1]));

    let ip_a = IPv4::new([0, 0, 0, 0]);
    let ip_b = IPv4::new([0, 0, 0, 1]);
    let ip_c = IPv4::new([0, 0, 0, 2]);
    let ip_router = IPv4::new([10, 0, 0, 2]);

    let mask_match_all = IPv4::new([255, 255, 255, 255]);

    let net1_gateway = link_router_net1.get_addr();
    let net2_gateway = link_router_net2.get_addr();

    let routing_table = vec![
        (ip_a, mask_match_all, 0, link_a.get_addr()),
        (ip_b, mask_match_all, 0, link_b.get_addr()),
        (ip_c, mask_match_all, 1, link_c.get_addr()),
    ];

    let ipv4_mask_any = IPv4::new([0, 0, 0, 0]);

    let device_a =
        Router::<&[u8], IPv4>::new_simple(link_a, net1_gateway, ipv4_mask_any, ipv4_mask_any, ip_a)
            .start();
    let device_b =
        Router::<&[u8], IPv4>::new_simple(link_b, net1_gateway, ipv4_mask_any, ipv4_mask_any, ip_b)
            .start();
    let device_c =
        Router::<&[u8], IPv4>::new_simple(link_c, net2_gateway, ipv4_mask_any, ipv4_mask_any, ip_c)
            .start();

    let router = Router::<&[u8], IPv4>::new(
        vec![link_router_net1, link_router_net2],
        routing_table,
        ip_router,
    )
    .start();

    device_a.send(&[0], ip_b).unwrap();

    device_a.send(&[1], ip_c).unwrap();

    device_c.send(&[2], ip_a).unwrap();

    device_c.send(&[3], ip_b).unwrap();

    device_c.send(&[4], ip_c).unwrap();

    device_b.send(&[5], ip_a).unwrap();

    device_b.send(&[6, 9], ip_router).unwrap();

    debug!("Sleeping for 5 seconds to allow for packets to arrive");
    std::thread::sleep(std::time::Duration::from_secs(5));

    println!("a {ip_a}:");
    while let Ok(Some(packet)) = device_a.try_recv() {
        println!("{packet:?}")
    }
    println!("b {ip_b}:");
    while let Ok(Some(packet)) = device_b.try_recv() {
        println!("{packet:?}")
    }
    println!("c {ip_c}:");
    while let Ok(Some(packet)) = device_c.try_recv() {
        println!("{packet:?}")
    }

    println!("router {ip_router}:");
    while let Ok(Some(packet)) = router.try_recv() {
        println!("{packet:?}")
    }

    info!("Stopping router");
    device_a.stop().unwrap();
    device_b.stop().unwrap();
    device_c.stop().unwrap();
    router.stop().unwrap();
    info!("Stopped router");
}
