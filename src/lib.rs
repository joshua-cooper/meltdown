//! A lightweight service manager to help with graceful shutdown of asynchronous applications.

#![forbid(unsafe_code)]
#![warn(
    missing_docs,
    clippy::all,
    clippy::pedantic,
    clippy::nursery,
    clippy::cargo
)]

use futures::{
    channel::oneshot::{self, Receiver, Sender},
    stream::FuturesUnordered,
    Stream,
};
use std::{
    future::Future,
    pin::Pin,
    task::{Context, Poll},
};

/// A token future that resolves once a [`Meltdown`] has been triggered.
pub struct Token {
    receiver: Receiver<()>,
}

impl Token {
    /// Creates a new [`Token`].
    #[must_use]
    const fn new(receiver: Receiver<()>) -> Self {
        Self { receiver }
    }
}

impl Future for Token {
    type Output = ();

    fn poll(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        Pin::new(&mut self.receiver).poll(cx).map(|_| ())
    }
}

/// A service.
pub trait Service<F> {
    /// Runs the service to completion.
    ///
    /// If the [`Token`] future provided to the service resolves, the service is expected to start
    /// a graceful shutdown.
    fn call(self, token: Token) -> F;
}

impl<T, F> Service<F> for T
where
    T: FnOnce(Token) -> F,
{
    fn call(self, token: Token) -> F {
        self(token)
    }
}

/// A service manager.
pub struct Meltdown<T> {
    senders: Vec<Sender<()>>,
    futures: FuturesUnordered<Pin<Box<dyn Future<Output = T> + Send>>>,
}

impl<T> Meltdown<T> {
    /// Creates a new instance of [`Meltdown`].
    #[must_use]
    pub fn new() -> Self {
        Self {
            senders: Vec::new(),
            futures: FuturesUnordered::new(),
        }
    }

    /// Registers a service.
    pub fn register<S, F>(&mut self, service: S) -> &mut Self
    where
        S: Service<F>,
        F: Future<Output = T> + Send + 'static,
    {
        let (sender, receiver) = oneshot::channel();
        self.senders.push(sender);
        self.futures
            .push(Box::pin(service.call(Token::new(receiver))));
        self
    }

    /// Triggers a meltdown.
    ///
    /// This will cause the [`Token`] futures of each running service to be resolved, signalling to
    /// start a graceful shutdown.
    pub fn trigger(&mut self) {
        for sender in self.senders.drain(..) {
            let _result = sender.send(());
        }
    }
}

impl<T> Default for Meltdown<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Stream for Meltdown<T> {
    type Item = T;

    fn poll_next(mut self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        Pin::new(&mut self.futures).poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::executor;

    #[test]
    fn can_register_and_run_services() {
        let mut meltdown = executor::block_on_stream(Meltdown::new());

        meltdown
            .register(|_| async { "service 1" })
            .register(|_| async { "service 2" })
            .register(|_| async { "service 3" });

        assert!(meltdown.next().is_some());
        assert!(meltdown.next().is_some());
        assert!(meltdown.next().is_some());
        assert!(meltdown.next().is_none());
    }

    #[test]
    fn can_trigger_meltdown() {
        let mut meltdown = executor::block_on_stream(Meltdown::new());

        meltdown
            .register(|t| async {
                t.await;
                2
            })
            .register(|_| async { 1 })
            .register(|_| async { 1 })
            .register(|t| async {
                t.await;
                2
            });

        assert_eq!(meltdown.next(), Some(1));
        assert_eq!(meltdown.next(), Some(1));

        meltdown.trigger();

        assert_eq!(meltdown.next(), Some(2));
        assert_eq!(meltdown.next(), Some(2));

        assert!(meltdown.next().is_none());
    }

    #[test]
    fn can_register_and_run_services_after_meltdowns() {
        let mut meltdown = executor::block_on_stream(Meltdown::new());

        meltdown.register(|_| async { 1 }).register(|t| async {
            t.await;
            2
        });

        assert_eq!(meltdown.next(), Some(1));

        meltdown.trigger();

        assert_eq!(meltdown.next(), Some(2));

        meltdown.register(|_| async { 3 }).register(|t| async {
            t.await;
            4
        });

        assert_eq!(meltdown.next(), Some(3));

        meltdown.trigger();

        assert_eq!(meltdown.next(), Some(4));

        assert!(meltdown.next().is_none());
    }
}
