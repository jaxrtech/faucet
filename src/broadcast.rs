use std::fmt::Debug;
use std::hash::Hash;
use std::sync::Arc;
use dashmap::{DashMap, Entry};
use tokio::sync::broadcast;
use tokio_util::sync::CancellationToken;
use tracing::instrument;
use crate::Faucet;

pub struct Broadcast<Id, T> {
    joined: Arc<broadcast::Sender<Id>>,
    receivers: Arc<DashMap<Id, Faucet<T>>>,
    completion: CancellationToken,
}

impl<Id: Clone, T: Clone> Clone for Broadcast<Id, T> {
    fn clone(&self) -> Self {
        Self {
            joined: self.joined.clone(),
            receivers: self.receivers.clone(),
            completion: self.completion.clone(),
        }
    }
}

impl<Id: PartialEq + Eq + Hash + Clone + Debug, T: Clone> Broadcast<Id, T> {
    pub fn on_joined(&self) -> broadcast::Receiver<Id> {
        self.joined.subscribe()
    }

    pub fn new_with_cancellation(cancellation: CancellationToken) -> Self {
        let (joined, _) = broadcast::channel(1);
        Self {
            joined: Arc::new(joined),
            receivers: Arc::new(DashMap::new()),
            completion: cancellation.child_token(),
        }
    }

    pub fn get_or_join(&self, id: Id, max_len: usize) -> Faucet<T> {
        match self.receivers.entry(id.clone()) {
            Entry::Occupied(entry) => entry.get().clone(),
            Entry::Vacant(entry) => {
                let faucet = Faucet::new_with_cancellation(max_len, self.completion.child_token());
                entry.insert(faucet.clone());
                self.joined.send(id).ok();
                faucet
            }
        }
    }

    pub fn push_or_kick(&self, id: &Id, value: &T) {
        if let Some(faucet) = self.receivers.get(id) {
            if faucet.try_push(value.clone()).is_err() {
                self.kick(id)
            }
        }
    }

    pub fn push_all_or_kick(&self, value: &T) {
        let kicked = self
            .receivers
            .iter()
            .filter_map(|x| {
                let (id, faucet) = x.pair();
                if faucet.try_push(value.clone()).is_ok() {
                    None
                } else {
                    Some(id.clone())
                }
            })
            .collect::<Vec<_>>();

        for id in kicked {
            self.kick(&id)
        }
    }

    #[instrument(skip(self))]
    pub fn kick(&self, id: &Id) {
        if let Some((_, faucet)) = self.receivers.remove(id) {
            faucet.end();
        }
    }

    pub async fn cancelled(&self) {
        self.completion.cancelled().await;
    }

    #[instrument(skip_all)]
    pub fn end(&self) {
        self.completion.cancel();
    }
}
