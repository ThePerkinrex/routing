use std::{collections::HashMap, sync::Arc};

use tokio::{task::JoinHandle, sync::RwLock};

#[derive(Debug)]
struct ProcessManagerInternal {
	processes: HashMap<u64, JoinHandle<()>>,
	available: Vec<u64>,
	next: u64,
}

impl ProcessManagerInternal {
	fn new() -> Self {
		Self {
			processes: HashMap::new(),
			available: vec![],
			next: 0,
		}
	}

	fn add_handle<F: FnOnce(u64) -> JoinHandle<()>>(&mut self, f: F) -> u64 {
		let to_add = self.available.pop().unwrap_or_else(|| {
			let res = self.next;
			self.next += 1;
			res
		});
		self.processes.insert(to_add, f(to_add));
		to_add
	}

	fn remove(&mut self, pid: u64) -> Option<JoinHandle<()>> {
		self.processes.remove(&pid).map(|handle| {
			if pid+1 == self.next {
				self.next -= 1;
				while self.available.last().map(|x| x == &(self.next-1)).unwrap_or(false) {
					self.available.pop();
					self.next -= 1;
				}
			}else{
				self.available.push(pid)
			}
			handle
		})
	}
}

impl Default for ProcessManagerInternal {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Default, Debug)]
pub struct ProcessManager {
	internal: Arc<RwLock<ProcessManagerInternal>>,
}

impl ProcessManager {
	pub fn new() -> Self {
		Self::default()
	}

	pub async fn add<F: FnOnce(u64) -> Fut + Send, Fut: std::future::Future<Output = ()> + Send + 'static>(&self, f: F) -> u64 {
		let internal = self.internal.clone();
		self.internal.write().await.add_handle(move |pid| {
			let fut = f(pid);
			tokio::spawn(async move {
				fut.await;
				internal.write().await.remove(pid);
			})
		})
	}

	pub async fn stop_process(&self, pid: u64) -> Result<(), tokio::task::JoinError> {
		if let Some(handle) = self.internal.write().await.remove(pid) {
			if !handle.is_finished() {
				handle.abort();
			}
			handle.await
		}else{
			Ok(())
		}
	}
}

