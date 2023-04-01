use std::{fmt::{Display, Formatter}, sync::Arc};

use tokio::sync::RwLock;

use crate::chassis::{NicHandle, self};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PortState {
	Up,
	Down,
	Unknown
}

pub enum PortKind {
	Ethernet
}

pub trait PortDisplay {
	fn name(&self, f: &mut Formatter) ->std::fmt::Result;
}

#[async_trait::async_trait]
pub trait Port: PortDisplay {
	async fn state(&self) -> PortState;
	async fn kind(&self) -> PortKind;
}

#[async_trait::async_trait]
pub trait Ports {
	type PortsIter: Iterator<Item = Box<dyn Port>>;
	async fn ports(&self) -> Self::PortsIter;
}

impl PortDisplay for (usize, chassis::switch::PortType, Arc<RwLock<NicHandle>>) {
    fn name(&self, f: &mut Formatter) ->std::fmt::Result {
        write!(f, "eth{}",self.0)
    }
}

#[async_trait::async_trait]
impl Port for (usize, crate::chassis::switch::PortType, Arc<RwLock<NicHandle>>) {
	async fn state(&self) -> PortState {
		if self.2.read().await.connected() {
			PortState::Up
		}else{
			PortState::Down
		}
	}
	async fn kind(&self) -> PortKind {
		PortKind::Ethernet
	}
}
