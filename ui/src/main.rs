use bevy::prelude::*;
use routing::{
    link::Interface,
    physical::{Ethernet, EthernetFrameReceived, EthernetPlugin, EthernetPort, EthernetFrameSent, EthernetEcho},
    EntityEvent,
};
use routing_lib::{
    link::ethernet::packet::EthernetPacket,
    mac::authority::{MacAdminAuthority, SequentialAuthority},
};

mod routing;

fn add_simple_port_net(mut cmd: Commands, mut tx: EventWriter<EthernetFrameSent>) {
    let entity = cmd.spawn(Ethernet {}).id();
    let mut auth = SequentialAuthority::new([0, 0, 1]);
    let mac = auth.get_addr();
    cmd.spawn(EthernetEcho).with_children(|child| {
        child.spawn((
            Interface::new("eth0".into()),
            EthernetPort::new(Some(entity), mac),
        ));
    });
    let other_mac = auth.get_addr();
    let other = cmd.spawn(()).with_children(|child| {
        child.spawn((
            Interface::new("eth1".into()),
            EthernetPort::new(Some(entity), other_mac),
        ));
    }).id();
    tx.send(EthernetFrameSent(EntityEvent(
        other,
        (EthernetPacket::new_ip_v4(mac, other_mac, vec![0x69]).unwrap(), "eth1".into()),
    )))
}

fn received_packets_printer(mut rx: EventReader<EthernetFrameReceived>) {
    for event in rx.iter() {
        info!("{event:#?}")
    }
}

fn main() {
    App::new()
        // .add_plugins(MinimalPlugins)
        .add_plugins(DefaultPlugins)
        .add_plugin(EthernetPlugin)
        .add_startup_system(add_simple_port_net)
        .add_system(received_packets_printer)
        .run();
}
