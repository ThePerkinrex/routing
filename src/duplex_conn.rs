#[derive(Clone)]
pub struct DuplexBarrage<P: Clone + Unpin> {
    pub tx: barrage::Sender<P>,
    pub rx: barrage::Receiver<P>,
}

impl<P: Clone + Unpin> DuplexBarrage<P> {
    pub fn bounded(bound: usize) -> Self {
        let (tx, rx) = barrage::bounded(bound);
        Self {
            tx, rx
        }
    }

    pub fn unbounded() -> Self {
        let (tx, rx) = barrage::unbounded();
        Self {
            tx, rx
        }
    }
}
