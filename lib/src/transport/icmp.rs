use std::{collections::HashMap, sync::Arc};

use either::Either;
use flume::{Receiver, RecvError, Sender};
use tokio::task::JoinSet;
use tracing::warn;

use crate::{
    chassis::{
        NetworkLayerId, NetworkTransportMessage, ProcessMessage, ReceptionResult, TransportLayerId,
        TransportLevelProcess,
    },
    network::ipv4::addr::IpV4Addr,
};

use self::packet::IcmpPacket;

pub mod packet;

type Duplex<Tx, Rx> = (Sender<Tx>, Arc<Receiver<Rx>>);

pub struct IcmpApi {
    echo_ip_v4: Duplex<(u16, u16, IpV4Addr), Receiver<()>>,
}

impl IcmpApi {
    pub async fn echo_ip_v4(&self, id: u16, seq: u16, ip: IpV4Addr) -> Option<()> {
        self.echo_ip_v4.0.send_async((id, seq, ip)).await.ok()?;
        let rx = self.echo_ip_v4.1.recv_async().await.ok()?;
        rx.recv_async().await.ok()
    }
}

pub struct IcmpProcess {
    echo_ip_v4: Duplex<Receiver<()>, (u16, u16, IpV4Addr)>,
    echo_data_ip_v4: HashMap<(u16, u16, IpV4Addr), Sender<()>>,
}

impl IcmpProcess {
    pub fn new() -> (Self, IcmpApi) {
        let (echo_ip_v4_internal_tx, echo_ip_v4_external_rx) = flume::unbounded();
        let (echo_ip_v4_external_tx, echo_ip_v4_internal_rx) = flume::unbounded();
        (
            Self {
                echo_ip_v4: (echo_ip_v4_internal_tx, Arc::new(echo_ip_v4_internal_rx)),
                echo_data_ip_v4: HashMap::new(),
            },
            IcmpApi {
                echo_ip_v4: (echo_ip_v4_external_tx, Arc::new(echo_ip_v4_external_rx)),
            },
        )
    }
}

pub enum ExtraMessage {
    EchoIpV4(Result<(u16, u16, IpV4Addr), RecvError>),
}

#[async_trait::async_trait]
impl TransportLevelProcess<TransportLayerId, NetworkLayerId, NetworkTransportMessage>
    for IcmpProcess
{
    async fn on_down_message(
        &mut self,
        msg: NetworkTransportMessage,
        down_id: NetworkLayerId,
        down_sender: &HashMap<
            NetworkLayerId,
            Sender<ProcessMessage<TransportLayerId, NetworkLayerId, NetworkTransportMessage>>,
        >,
    ) {
        match msg {
            NetworkTransportMessage::IPv4(addr, payload) => {
                if let Some(msg) = IcmpPacket::from_vec(&payload) {
                    match msg {
                        IcmpPacket::EchoRequest { id, seq } => {
                            let _ = down_sender[&down_id]
                                .send_async(ProcessMessage::Message(
                                    TransportLayerId::Icmp,
                                    NetworkTransportMessage::IPv4(
                                        addr,
                                        IcmpPacket::EchoReply { id, seq }.to_vec(),
                                    ),
                                ))
                                .await;
                        }
                        IcmpPacket::EchoReply { id, seq } => {
                            if let Some(tx) = self.echo_data_ip_v4.remove(&(id, seq, addr)) {
                                let _ = tx.send_async(()).await;
                            }
                        }
                    }
                }
            }
        }
    }
    async fn setup(
        &mut self,
        join_set: &mut JoinSet<
            Either<
                Result<
                    ProcessMessage<NetworkLayerId, TransportLayerId, NetworkTransportMessage>,
                    RecvError,
                >,
                Self::Extra,
            >,
        >,
    ) {
        let rx = self.echo_ip_v4.1.clone();
        join_set.spawn(async move { Either::Right(ExtraMessage::EchoIpV4(rx.recv_async().await)) });
    }
    type Extra = ExtraMessage;
    async fn on_extra_message(
        &mut self,
        msg: Self::Extra,
        down_sender: &HashMap<
            NetworkLayerId,
            Sender<ProcessMessage<TransportLayerId, NetworkLayerId, NetworkTransportMessage>>,
        >,
        join_set: &mut JoinSet<
            Either<
                Result<
                    ProcessMessage<NetworkLayerId, TransportLayerId, NetworkTransportMessage>,
                    RecvError,
                >,
                Self::Extra,
            >,
        >,
    ) {
        match msg {
            ExtraMessage::EchoIpV4(msg) => match msg {
                Ok(msg) => {
                    let (tx, rx) = flume::bounded(1);
                    self.echo_data_ip_v4.insert(msg, tx);
                    let _ = self.echo_ip_v4.0.send_async(rx).await;
                    let rx = self.echo_ip_v4.1.clone();
                    join_set.spawn(async move {
                        Either::Right(ExtraMessage::EchoIpV4(rx.recv_async().await))
                    });
                    if let Some(sender) = down_sender.get(&NetworkLayerId::Ipv4) {
                        let (id, seq, addr) = msg;
                        let _ = sender
                            .send_async(ProcessMessage::Message(
                                TransportLayerId::Icmp,
                                NetworkTransportMessage::IPv4(
                                    addr,
                                    IcmpPacket::EchoRequest { id, seq }.to_vec(),
                                ),
                            ))
                            .await;
                    }
                }
                Err(RecvError::Disconnected) => warn!("Handler echo ip v4 disconnected"),
            },
        }
    }
}
