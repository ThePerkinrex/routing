use bevy::{
    prelude::{
        Component, Entity, EventReader, EventWriter, HierarchyQueryExt, Parent, Plugin, Query,
    },
    utils::HashMap,
};
use routing_lib::{link::ethernet::packet::EthernetPacket, mac::Mac};

use super::{link::Interface, EntityEvent};

#[derive(Debug)]
pub struct EthernetFrameReceived(pub EntityEvent<(EthernetPacket, String)>);

pub struct EthernetFrameSent(pub EntityEvent<(EthernetPacket, String)>);

pub struct EthernetEvent(pub EntityEvent<EthernetPacket>);

#[derive(Debug, Component)]
pub struct EthernetPort {
    connection: Option<Entity>,
    mac: Mac,
}

impl EthernetPort {
    pub const fn new(connection: Option<Entity>, mac: Mac) -> Self {
        Self { connection, mac }
    }
}

#[derive(Debug, Component)]
pub struct Ethernet {}

fn ethernet_port_receiver_system(
    ports: Query<(Entity, &EthernetPort, &Interface)>,
    parents: Query<&Parent>,
    mut up_tx: EventWriter<EthernetFrameReceived>,
    mut net: EventReader<EthernetEvent>,
) {
    let mut hm = HashMap::<_, Vec<_>>::with_capacity(net.len());
    for (e, frame) in net
        .iter()
        .map(|EthernetEvent(EntityEvent(e, frame))| (e, frame))
    {
        hm.entry(*e).or_default().push(frame)
    }
    up_tx.send_batch(
        ports
            .iter()
            .filter_map(|(a, b, c)| b.connection.map(|x| (a, x, b.mac, c)))
            .flat_map(|(port_entity, conn, mac, iface)| {
                let iface: String = iface.name().into();
                let hm_conn = hm.get(&conn);
                parents
                    .iter_ancestors(port_entity)
                    .next()
                    .into_iter()
                    .flat_map(move |parent| {
                        let iface = iface.clone();
                        hm_conn
                            .into_iter()
                            .flat_map(|x| x.iter())
                            .filter(move |frame| {
                                frame.get_dest() == mac || frame.get_dest().is_broadcast()
                            })
                            .map(move |frame| {
                                EthernetFrameReceived(EntityEvent(
                                    parent,
                                    ((*frame).clone(), iface.clone()),
                                ))
                            })
                    })
            }),
    );
}

fn ethernet_port_sender_system(
    ports: Query<(Entity, &EthernetPort, &Interface)>,
    parents: Query<&Parent>,
    mut rx: EventReader<EthernetFrameSent>,
    mut net: EventWriter<EthernetEvent>,
) {
    net.send_batch(
        rx.iter()
            .filter_map(|EthernetFrameSent(EntityEvent(entity, (frame, port)))| {
                ports
                    .iter()
                    .find(|(pe, _, iface)| {
                        iface.name() == port && parents.iter_ancestors(*pe).any(|x| x == *entity)
                    })
                    .and_then(|(_, p, _)| p.connection)
                    .map(|conn| EthernetEvent(EntityEvent(conn, frame.clone())))
            }),
    )
}

#[derive(Debug, Component)]
pub struct EthernetEcho;

fn echo(
    echo_entity: Query<&EthernetEcho>,
    mut rx: EventReader<EthernetFrameReceived>,
    mut tx: EventWriter<EthernetFrameSent>,
) {
    tx.send_batch(rx.iter().filter_map(|EthernetFrameReceived(EntityEvent(e, (p, iface)))| if echo_entity.contains(*e) {
        let mut p = p.clone();
        p.swap_addr();
        Some(EthernetFrameSent(EntityEvent(*e, (p, iface.clone()))))
    }else{None}))
}

pub struct EthernetPlugin;

impl Plugin for EthernetPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.add_event::<EthernetFrameReceived>()
            .add_event::<EthernetFrameSent>()
            .add_event::<EthernetEvent>()
            .add_system(ethernet_port_receiver_system)
            .add_system(ethernet_port_sender_system)
            .add_system(echo);
    }
}
