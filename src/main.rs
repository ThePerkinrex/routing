use crate::{ethernet::EthernetNet, router::Router, tcp_ip::Ip};

mod ethernet;
mod router;
mod tcp_ip;

fn main() {
    println!("Hello, world!");
    let net1 = EthernetNet::new();
    let link_a = net1.get_link([0, 0, 0, 0, 0, 0]);
    let link_b = net1.get_link([0, 0, 0, 0, 0, 1]);
    let link_router_net1 = net1.get_link([0, 0, 0, 0, 0, 2]);
    let net2 = EthernetNet::new();
    let link_router_net2 = net2.get_link([0, 0, 0, 0, 0, 0]);
    let link_c = net2.get_link([0, 0, 0, 0, 0, 1]);

    let ip_a = 0;
    let ip_b = 1;
    let ip_c = 2;

    Router::<&[u8]>::new(
        vec![link_router_net1, link_router_net2],
        vec![
            (ip_a, 0xffffffff, 0, [0, 0, 0, 0, 0, 0]),
            (ip_b, 0xffffffff, 0, [0, 0, 0, 0, 0, 1]),
            (ip_c, 0xffffffff, 1, [0, 0, 0, 0, 0, 1]),
        ],
    )
    .start();

    let net1_gateway = [0, 0, 0, 0, 0, 2];
    let net2_gateway = [0, 0, 0, 0, 0, 0];

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
                payload: &[3],
            },
            net2_gateway,
        )
        .unwrap();

    link_b
        .send(
            Ip {
                origin: ip_b,
                dest: ip_a,
                payload: &[3],
            },
            net2_gateway,
        )
        .unwrap();

    println!("a:");
    while let Ok(Some(packet)) = link_a.try_recv() {
        println!("{packet:?}")
    }
    println!("b:");
    while let Ok(Some(packet)) = link_b.try_recv() {
        println!("{packet:?}")
    }
    println!("c:");
    while let Ok(Some(packet)) = link_c.try_recv() {
        println!("{packet:?}")
    }
}
