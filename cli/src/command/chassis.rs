use std::sync::Arc;

use tokio::sync::RwLock;

use crate::{
    chassis::{ChassisData, ChassisManager},
    ctrlc::CtrlC,
};

use super::ParsedCommand;
pub mod arp;
pub mod ip_v4;
pub mod link;

#[async_trait::async_trait]
pub trait ParsedChassisCommand<Args> {
    async fn run(
        &mut self,
        args: Args,
        chassis_mgr: Arc<RwLock<ChassisManager>>,
        ctrlc: &CtrlC,
        name: String,
    ) -> bool
    where
        Args: 'async_trait;
}

#[async_trait::async_trait]
pub trait ParsedChassisCommandRwLock<Args> {
    async fn run(
        &mut self,
        args: Args,
        ctrlc: &CtrlC,
        name: String,
        chassis: &RwLock<ChassisData>,
    ) -> bool
    where
        Args: 'async_trait;
}

#[async_trait::async_trait]
impl<Args: Send, T: ParsedChassisCommandRwLock<Args> + Send> ParsedChassisCommand<Args> for T {
    async fn run(
        &mut self,
        args: Args,
        chassis_mgr: Arc<RwLock<ChassisManager>>,
        ctrlc: &CtrlC,
        name: String,
    ) -> bool
    where
        Args: 'async_trait,
    {
        let guard = chassis_mgr.read().await;
        let data = guard.get(&name).unwrap();
        self.run(args, ctrlc, name, data).await
    }
}

#[async_trait::async_trait]
pub trait ParsedChassisCommandRead<Args> {
    async fn run(&mut self, args: Args, ctrlc: &CtrlC, name: String, chassis: &ChassisData)
        -> bool;
}

#[async_trait::async_trait]
impl<Args: Send, T: ParsedChassisCommandRead<Args> + Send> ParsedChassisCommandRwLock<Args> for T {
    async fn run(
        &mut self,
        args: Args,
        ctrlc: &CtrlC,
        name: String,
        chassis: &RwLock<ChassisData>,
    ) -> bool
    where
        Args: 'async_trait,
    {
        let guard = chassis.read().await;
        self.run(args, ctrlc, name, &guard).await
    }
}

impl<Args: clap::Parser + 'static, T: ParsedChassisCommand<Args>> ParsedCommand<Args, String, bool>
    for T
{
    fn run<'life0, 'life1, 'async_trait>(
        &'life0 mut self,
        args: Args,
        chassis_mgr: Arc<RwLock<ChassisManager>>,
        ctrlc: &'life1 CtrlC,
        name: String,
    ) -> core::pin::Pin<
        Box<dyn core::future::Future<Output = bool> + core::marker::Send + 'async_trait>,
    >
    where
        'life0: 'async_trait,
        'life1: 'async_trait,
        Self: 'async_trait,
    {
        self.run::<'life0, 'life1, 'async_trait>(args, chassis_mgr, ctrlc, name)
    }
}

#[derive(Debug, clap::Parser)]
pub struct Exit;

#[async_trait::async_trait]
impl ParsedChassisCommand<Self> for Exit {
    async fn run(&mut self, _: Self, _: Arc<RwLock<ChassisManager>>, _: &CtrlC, _: String) -> bool {
        true
    }
}

// pub fn register_commands<'a, 'b, 'c>(
//     command_mgr: &'a mut CommandManager<(&'b ChassisData, &'c str), Result<bool, clap::Error>>,
// ) {
//     command_mgr.register::<PCmd<_, _, _, _>, _, _>("exit", Exit);
// }
