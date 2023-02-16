use tracing::info;

use crate::{
    ethernet::{EthernetNet, PhysicalAddr},
    ip::{IPv4, Ip},
    router::Router,
};

mod ethernet;
mod ip;
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
    let (stop_tx, join_handle) = Router::<&[u8], IPv4>::new(
        vec![link_router_net1, link_router_net2],
        routing_table,
        ip_router,
    )
    .start();

    link_a
        .send(
            Ip {
                origin: ip_a,
                dest: ip_b,
                payload: &[0],
            },
            net1_gateway,
        )
        .unwrap();

    link_a
        .send(
            Ip {
                origin: ip_a,
                dest: ip_c,
                payload: &[1],
            },
            net1_gateway,
        )
        .unwrap();

    link_c
        .send(
            Ip {
                origin: ip_c,
                dest: ip_c,
                payload: &[2],
            },
            net2_gateway,
        )
        .unwrap();

    link_c
        .send(
            Ip {
                origin: ip_c,
                dest: ip_b,
                payload: &[3],
            },
            net2_gateway,
        )
        .unwrap();

    link_c
        .send(
            Ip {
                origin: ip_c,
                dest: ip_c,
                payload: &[4],
            },
            net2_gateway,
        )
        .unwrap();

    link_b
        .send(
            Ip {
                origin: ip_b,
                dest: ip_a,
                payload: &[5],
            },
            net1_gateway,
        )
        .unwrap();

    link_b
        .send(
            Ip {
                origin: ip_b,
                dest: ip_router,
                payload: &[6, 9],
            },
            net1_gateway,
        )
        .unwrap();

    println!("a {ip_a} {}:", link_a.get_addr());
    while let Ok(Some(packet)) = link_a.try_recv() {
        println!("{packet:?}")
    }
    println!("b {ip_b} {}:", link_b.get_addr());
    while let Ok(Some(packet)) = link_b.try_recv() {
        println!("{packet:?}")
    }
    println!("c {ip_c} {}:", link_c.get_addr());
    while let Ok(Some(packet)) = link_c.try_recv() {
        println!("{packet:?}")
    }

    info!("Stopping router");
    stop_tx.send(()).unwrap();
    let _router = join_handle.join().unwrap();
    info!("Stopped router");
}
