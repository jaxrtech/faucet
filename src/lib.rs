use std::fmt::Debug;
use std::ops::ControlFlow;
use std::sync::Arc;

use tokio::select;
use tokio_util::sync::CancellationToken;

/// A back-pressured queue limited in size that can be drained after signaling
/// completion.
///
/// This queue implementation has the following characteristics:
///  * Based on [`deadqueue::limited::Queue`]
///  * Has limited capacity with back-pressure on push
///  * Can signal completion by calling [`end()`] or by providing a cancellation
///    token and calling [`cancel()`] on it
///  * Once completion is signaled, no additional values can be pushed while
///    any values remaining in the queue can be drained
///
/// [`end()`]: Faucet::end
/// [`cancel()`]: CancellationToken::cancel
///
#[derive(Debug)]
pub struct Faucet<T> {
    queue: Arc<deadqueue::limited::Queue<T>>,
    completion: CancellationToken,
}

impl<T> Clone for Faucet<T> {
    fn clone(&self) -> Self {
        Self {
            queue: self.queue.clone(),
            completion: self.completion.clone(),
        }
    }
}

impl<T> Faucet<T> {
    /// Creates a new faucet with a maximum queue length.
    #[must_use]
    pub fn new(max_len: usize) -> Self {
        Self {
            queue: Arc::new(deadqueue::limited::Queue::new(max_len)),
            completion: CancellationToken::new(),
        }
    }

    /// Creates a new faucet with a maximum queue length and a cancellation
    /// source.
    ///
    /// Providing an existing cancellation token is useful when you have a
    /// "parent" cancellation token.
    ///
    /// Cancelling the token will prevent any additional values from
    /// being pushed onto the queue, and will drain any values already in the
    /// queue.
    #[must_use]
    pub fn new_with_cancellation(max_len: usize, cancellation: CancellationToken) -> Self {
        Self {
            queue: Arc::new(deadqueue::limited::Queue::new(max_len)),
            completion: cancellation,
        }
    }

    /// Cancels the faucet, preventing any additional values from being pushed
    /// onto the queue. Any values already in the queue will be drained.
    pub fn end(&self) {
        self.completion.cancel();
    }

    /// Returns true if the faucet has been cancelled and has no more values
    /// remaining in the queue to be drained.
    #[must_use]
    pub fn is_finished(&self) -> bool {
        self.queue.is_empty() && self.completion.is_cancelled()
    }

    /// Returns true if the faucet is either: (a) accepting values, or (b) is
    /// cancelled but has not been fully drained.
    #[must_use]
    pub fn is_pending(&self) -> bool {
        !self.is_finished()
    }

    /// Returns true if the faucet has been cancelled and will not accept any
    /// additional values pushed onto the queue.
    #[must_use]
    pub fn is_cancelled(&self) -> bool {
        self.completion.is_cancelled()
    }

    /// Pushes a value onto the queue or waits until space is available.
    pub async fn push(&self, value: T) -> ControlFlow<(), ()> {
        select! {
            _ = self.completion.cancelled() => {
                ControlFlow::Break(())
            },
            _ = self.queue.push(value) => {
                ControlFlow::Continue(())
            }
        }
    }

    /// Attempts to push a value onto the queue, returning `Err(value)` if the
    /// queue is full or has been cancelled.
    pub async fn try_push(&self, value: T) -> Result<(), T> {
        if self.completion.is_cancelled() {
            return Err(value);
        }

        self.queue.try_push(value)
    }

    /// Attempts to pop a value from the queue, returning `None` if the queue is
    /// has been cancelled and finished draining.
    pub async fn next(&self) -> Option<T> {
        select! {
            biased;
            _ = self.completion.cancelled() => {
                self.queue.try_pop()
            },
            x = self.queue.pop() => {
                Some(x)
            }
        }
    }

    /// Attempts to pop a value from the queue, returning `None` if the queue is
    /// currently empty.
    #[must_use]
    pub fn try_pop(&self) -> Option<T> {
        self.queue.try_pop()
    }

    /// The number of items currently stored in the queue.
    pub fn len(&self) -> usize {
        self.queue.len()
    }

    /// The maximum number of items that can be stored in the queue.
    pub fn capacity(&self) -> usize {
        self.queue.capacity()
    }
}
