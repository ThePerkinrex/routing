use std::{
    io::Write,
    ops::Deref,
    path::PathBuf,
    sync::{Arc, Barrier},
};

use tokio::sync::{AcquireError, RwLock, Semaphore};
use tracing::error;

use crate::{chassis::ChassisManager, command::ParsedCommand, ctrlc::CtrlC};

#[derive(Debug, clap::Parser)]
pub struct Source {
    path: PathBuf,
}

pub struct SourceCommand {
    lines: tokio::sync::mpsc::UnboundedSender<Vec<String>>,
}

#[async_trait::async_trait]
impl<R: Default + Send, E: Send + 'static> ParsedCommand<Source, E, R> for &SourceCommand {
    async fn run(
        &mut self,
        Source { path }: Source,
        _: Arc<RwLock<ChassisManager>>,
        _: &CtrlC,
        _: E,
    ) -> R {
        match tokio::fs::read_to_string(path).await {
            Ok(file) => {
                let _ = self.lines.send(
                    file.lines()
                        .map(ToString::to_string)
                        .rev()
                        .collect::<Vec<_>>(),
                );
            }
            Err(e) => error!("Read error: {e}"),
        }

        Default::default()
    }
}

pub struct CliCommands {
    number_of_commands: Arc<Semaphore>,
    commands: Arc<RwLock<Vec<String>>>,
    barrier: Arc<Barrier>,
}

impl CliCommands {
    pub async fn next(&self) -> Result<CommandHandle, AcquireError> {
        self.number_of_commands.acquire().await?.forget();
        Ok(CommandHandle {
            cmd: self.commands.write().await.pop().unwrap(),
            barrier: self.barrier.clone(),
            number_of_commands: self.number_of_commands.clone(),
        })
    }
}

pub struct CommandHandle {
    cmd: String,
    barrier: Arc<Barrier>,
    number_of_commands: Arc<Semaphore>,
}

impl Deref for CommandHandle {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.cmd
    }
}

impl Drop for CommandHandle {
    fn drop(&mut self) {
        if self.number_of_commands.available_permits() == 0 {
            self.barrier.wait();
        }
    }
}

pub fn cli() -> (CliCommands, SourceCommand) {
    let (tx2, mut rx2) = tokio::sync::mpsc::unbounded_channel();
    let cmd = SourceCommand { lines: tx2.clone() };
    let sem = Arc::new(Semaphore::new(0));
    let sem_clone = sem.clone();
    let commands = Arc::new(RwLock::new(Vec::new()));
    let commands_clone = commands.clone();
    let barrier = Arc::new(Barrier::new(2));
    let barrier_clone = barrier.clone();
    tokio::task::spawn(async move {
        while let Some(data) = rx2.recv().await {
            commands_clone.write().await.extend_from_slice(&data);
            sem.add_permits(data.len());
        }
    });
    std::thread::spawn(move || {
        let lock = std::io::stdin();
        let mut buf = String::new();
        print!(" > ");
        std::io::stdout().flush().unwrap();
        while lock.read_line(&mut buf).is_ok() {
            if !buf.trim().is_empty() {
                tx2.send(vec![buf.clone()]).unwrap();
                barrier_clone.wait();
            }
            buf.clear();
            print!(" > ");
            std::io::stdout().flush().unwrap();
        }
        error!("CLI Reader finishing")
    });
    (
        CliCommands {
            number_of_commands: sem_clone,
            commands,
            barrier,
        },
        cmd,
    )
}
