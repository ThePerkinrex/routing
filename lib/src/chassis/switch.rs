use std::{sync::{Arc}};

use tokio::{task::{JoinSet, JoinHandle}, sync::RwLock};

use crate::link::ethernet::{nic::Nic, packet::EthernetPacket};

use super::NicHandle;

enum SwitchMessage {
	Connect,
	Disconnect,
	ConnectNetwork((barrage::Sender<EthernetPacket>, barrage::Receiver<EthernetPacket>)),
	NicHandleError(flume::RecvError),
	EthernetFrame(EthernetPacket)
}

#[derive(Default)]
pub struct Switch {
	link_layer_processes: Vec<(JoinHandle<()>, Arc<RwLock<NicHandle>>)>,
}

impl Switch {
	pub fn add_nic(&mut self, nic: Nic) -> Arc<RwLock<NicHandle>> {
		let (conn_tx, conn_rx) = flume::unbounded();
        let (dconn_tx, dconn_rx) = flume::unbounded();
        let (conn_net_tx, conn_net_rx) = flume::unbounded();
        let (conn_reply_tx, conn_reply_rx) = flume::unbounded();
        let (dconn_reply_tx, dconn_reply_rx) = flume::unbounded();
        let (conn_net_reply_tx, conn_net_reply_rx) = flume::unbounded();
        let res = Arc::new(RwLock::new(NicHandle {
            connected: nic.is_up(),
            disconnect: (dconn_tx, dconn_reply_rx),
            connect: (conn_tx, conn_reply_rx),
            connect_to_net: (conn_net_tx, conn_net_reply_rx),
        }));
		self.link_layer_processes.push((tokio::spawn(async move {
			let conn_rx = Arc::new(conn_rx);
			let conn_task = move || {
				let rx = conn_rx.clone();
				async move { rx.recv_async().await.map_or_else(SwitchMessage::NicHandleError, |()| SwitchMessage::Connect)}
			};
			let dconn_rx = Arc::new(dconn_rx);
			let dconn_task = move || {
				let rx = dconn_rx.clone();
				async move { rx.recv_async().await.map_or_else(SwitchMessage::NicHandleError, |()| SwitchMessage::Disconnect)}
			};
			let conn_net_rx = Arc::new(conn_net_rx);
			let conn_net_task = move || {
				let rx = conn_net_rx.clone();
				async move { rx.recv_async().await.map_or_else(SwitchMessage::NicHandleError, SwitchMessage::ConnectNetwork)}
			};
			let (conn, mac) = nic.split();
			let mut join_set = JoinSet::new();
			join_set.spawn(conn_task());
			join_set.spawn(dconn_task());
			join_set.spawn(conn_net_task());
			let mut conn = conn.map(|(tx, rx)| (tx, Arc::new(rx)));
			loop {
				match join_set.join_next().await {
					Some(Ok(x)) => todo!(),
					Some(Err(e)) => todo!(),
					None => break,
				}
			}
		}), res.clone()));
		res
	}
}