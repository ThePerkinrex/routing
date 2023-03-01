use std::collections::HashMap;

use either::Either;
use flume::Sender;
use tokio::task::JoinSet;

use crate::chassis::{TransportLevelProcess, TransportLayerId, NetworkLayerId, NetworkTransportMessage, ProcessMessage, ReceptionResult};

pub mod packet;

pub struct IcmpApi {

}

pub struct IcmpProcess {}

#[async_trait::async_trait]
impl TransportLevelProcess<TransportLayerId, NetworkLayerId, NetworkTransportMessage> for IcmpProcess{
	async fn on_down_message(
        &mut self,
        msg: NetworkTransportMessage,
        down_id: NetworkLayerId,
        down_sender: &HashMap<NetworkLayerId, Sender<ProcessMessage<TransportLayerId, NetworkLayerId, NetworkTransportMessage>>>,
    ) {
		todo!()
	}
    async fn setup(
        &mut self,
        join_set: &mut JoinSet<
            Either<ReceptionResult<ProcessMessage<NetworkLayerId, TransportLayerId, NetworkTransportMessage>>, Self::Extra>,
        >,
    ) {
    }
    type Extra = ();
    async fn on_extra_message(
        &mut self,
        msg: Self::Extra,
        down_sender: &HashMap<NetworkLayerId, Sender<ProcessMessage<TransportLayerId, NetworkLayerId, NetworkTransportMessage>>>,
        join_set: &mut JoinSet<
            Either<ReceptionResult<ProcessMessage<NetworkLayerId, TransportLayerId, NetworkTransportMessage>>, Self::Extra>,
        >,
    ) {
    }
}