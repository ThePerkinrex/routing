use std::{collections::HashMap, marker::PhantomData, sync::Arc};

use tokio::sync::RwLock;
use tracing::warn;

use crate::{chassis::ChassisManager, ctrlc::CtrlC};

pub mod chassis;
pub mod general;

#[async_trait::async_trait]
pub trait Command<Extra, Ret> {
    async fn run_cmd(
        &mut self,
        args: &[&str],
        chassis_manager: Arc<RwLock<ChassisManager>>,
        ctrlc: &CtrlC,
        extra: Extra,
    ) -> Ret;
}

#[async_trait::async_trait]
pub trait ParsedCommand<Args: clap::Parser, Extra, Ret> {
    async fn run(
        &mut self,
        args: Args,
        chassis_manager: Arc<RwLock<ChassisManager>>,
        ctrlc: &CtrlC,
        extra: Extra,
    ) -> Ret
    where
        Extra: 'async_trait;
}

pub struct PCmd<Args, E, R, T> {
    t: T,
    _p: PhantomData<(Args, E, R)>,
}

impl<Args, E, R, T> From<T> for PCmd<Args, E, R, T>
where
    Args: clap::Parser,
    T: ParsedCommand<Args, E, R>,
{
    fn from(value: T) -> Self {
        Self {
            t: value,
            _p: PhantomData,
        }
    }
}

#[async_trait::async_trait]
impl<Args, Extra, Ret, T> Command<Extra, Result<Ret, clap::Error>> for PCmd<Args, Extra, Ret, T>
where
    Args: clap::Parser + Send,
    Extra: Send,
    Ret: Send,
    T: ParsedCommand<Args, Extra, Ret> + Send,
{
    async fn run_cmd(
        &mut self,
        args: &[&str],
        chassis_manager: Arc<RwLock<ChassisManager>>,
        ctrlc: &CtrlC,
        extra: Extra,
    ) -> Result<Ret, clap::Error> {
        let args = Args::try_parse_from(args)?;
        Ok(self.t.run(args, chassis_manager, ctrlc, extra).await)
    }
}

pub struct CommandManager<Extra, Ret> {
    commands: HashMap<String, Box<dyn Command<Extra, Ret> + Send>>,
}

impl<E: Send, R: Send> CommandManager<E, R> {
    pub fn new() -> Self {
        Self {
            commands: Default::default(),
        }
    }

    pub fn register<C: Command<E, R> + Send + 'static, I: Into<C>, S: Into<String>>(
        &mut self,
        name: S,
        command: I,
    ) {
        self.register_owned(name.into(), command)
    }

    pub fn register_owned<C: Command<E, R> + Send + 'static, I: Into<C>>(
        &mut self,
        name: String,
        command: I,
    ) {
        self.commands.insert(name, Box::new(command.into()));
    }

    pub async fn call(
        &mut self,
        args: &[&str],
        chassis_manager: Arc<RwLock<ChassisManager>>,
        ctrlc: &CtrlC,
        extra: E,
    ) -> Option<R> {
        if let Some(c) = self.commands.get_mut(args[0]) {
            Some(c.run_cmd(args, chassis_manager, ctrlc, extra).await)
        } else {
            warn!("Command `{}` not found, available options:", args[0]);
            for name in self.commands.keys() {
                warn!("\t{name}");
            }
            None
        }
    }
}
