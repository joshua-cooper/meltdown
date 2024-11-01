use alloc::sync::Arc;
use core::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};
use futures_channel::oneshot;
use futures_util::{future::Shared, FutureExt};
use std::sync::Mutex;

/// A token used to signal when to begin shutting down.
///
/// All clones of a token will be triggered at the same time once [`Token::trigger`] is called.
///
/// # Examples
///
/// ```
/// # pollster::block_on(async {
/// use meltdown::Token;
///
/// let token = Token::new();
///
/// std::thread::spawn({
///     let token = token.clone();
///
///     move || {
///         // Do some work before triggering.
///         // ...
///
///         token.trigger();
///     }
/// });
///
/// // This will wait until the token is triggered.
/// token.await;
/// # });
/// ```
#[derive(Clone)]
pub struct Token {
    sender: Arc<Mutex<Option<oneshot::Sender<()>>>>,
    receiver: Shared<oneshot::Receiver<()>>,
}

impl Token {
    /// Creates a new token.
    #[must_use]
    pub fn new() -> Self {
        let (sender, receiver) = oneshot::channel();

        Self {
            sender: Arc::new(Mutex::new(Some(sender))),
            receiver: receiver.shared(),
        }
    }

    /// Triggers this token.
    ///
    /// All pending futures for this token and its clones will be woken and resolved immediately.
    pub fn trigger(&self) {
        if let Ok(Some(sender)) = self.sender.lock().map(|mut guard| guard.take()) {
            let _ = sender.send(());
        }
    }
}

impl Default for Token {
    fn default() -> Self {
        Self::new()
    }
}

impl Future for Token {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        match Pin::new(&mut self.receiver).poll(cx) {
            Poll::Ready(_) => Poll::Ready(()),
            Poll::Pending => Poll::Pending,
        }
    }
}
