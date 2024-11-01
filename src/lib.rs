//! A lightweight service manager to help with graceful shutdown of asynchronous applications.
//!
//! # Overview
//!
//! Meltdown makes it easy to manage multiple long-running services and coordinate their graceful
//! shutdown. A service can be any async task that needs to run continuously and can respond to a
//! shutdown signal.
//!
//! Meltdown is runtime-agnostic and works with any async runtime. It has minimal dependencies,
//! only requiring [`futures_util`] and [`futures_channel`] for core async primitives.
//!
//! # Creating Services
//!
//! The simplest way to create a service is with an async function that takes a [`Token`]:
//!
//! ```
//! use meltdown::Token;
//!
//! async fn my_service(token: Token) {
//!     println!("Service starting...");
//!
//!     // Run until shutdown is triggered
//!     token.await;
//!
//!     println!("Service shutting down...");
//! }
//! ```
//!
//! For more complex services, you can implement the [`Service`] trait directly.
//!
//! # Managing Services
//!
//! Use [`Meltdown`] to register and manage your services:
//!
//! ```
//! # pollster::block_on(async {
//! use meltdown::Meltdown;
//!
//! let mut meltdown = Meltdown::new()
//!     .register(|_| async {
//!         // Completes immediately.
//!         1
//!     })
//!     .register(|token| async {
//!         // Waits for a shutdown trigger.
//!         token.await;
//!         2
//!     });
//!
//! if let Some(id) = meltdown.next().await {
//!     println!("{id} stopped, shutting down");
//!     meltdown.trigger();
//! }
//!
//! while let Some(id) = meltdown.next().await {
//!     println!("{id} stopped");
//! }
//! # })
//! ```

extern crate alloc;

#[cfg(feature = "catch-panic")]
pub mod catch_panic;
#[cfg(feature = "tagged")]
pub mod tagged;

mod service;
mod token;

pub use self::{service::Service, token::Token};

use alloc::boxed::Box;
use core::{future::Future, pin::Pin};
use futures_util::{stream::FuturesUnordered, Stream, StreamExt};

/// An asynchronous service manager.
///
/// # Examples
///
/// ```
/// # pollster::block_on(async {
/// use meltdown::Meltdown;
///
/// let mut meltdown = Meltdown::new()
///     .register(|_| async {
///         // Completes immediately.
///         1
///     })
///     .register(|token| async {
///         // Waits for a shutdown trigger.
///         token.await;
///         2
///     });
///
/// if let Some(id) = meltdown.next().await {
///     println!("{id} stopped, shutting down");
///     meltdown.trigger();
/// }
///
/// while let Some(id) = meltdown.next().await {
///     println!("{id} stopped");
/// }
/// # })
/// ```
pub struct Meltdown<T> {
    token: Token,
    futures: FuturesUnordered<Pin<Box<dyn Future<Output = T> + Send + 'static>>>,
}

impl<T> Meltdown<T> {
    /// Creates a new meltdown instance.
    #[must_use]
    pub fn new() -> Self {
        Self {
            token: Token::new(),
            futures: FuturesUnordered::new(),
        }
    }

    /// Returns a reference to the global token.
    ///
    /// Triggering this token is equivalent to calling [`Meltdown::trigger`]. The returned token
    /// can be cloned and used to trigger a meltdown in other parts of the program, for example, in
    /// another thread.
    pub const fn token(&self) -> &Token {
        &self.token
    }

    /// Registers a new service.
    #[must_use]
    pub fn register<S>(self, service: S) -> Self
    where
        S: Service,
        S::Future: Future<Output = T> + Send + 'static,
    {
        self.futures.push(Box::pin(service.run(self.token.clone())));
        self
    }

    /// Triggers a meltdown.
    ///
    /// This will call [`Token::trigger`] on the tokens passed to each managed service, signalling
    /// to begin a graceful shutdown.
    pub fn trigger(&self) {
        self.token.trigger();
    }

    /// Returns the result of the next service to shut down.
    ///
    /// If there are no more services left, `None` is returned.
    ///
    /// Note that this method must be called in order to drive the inner service futures to
    /// completion.
    pub async fn next(&mut self) -> Option<T> {
        StreamExt::next(self).await
    }
}

impl<T> Default for Meltdown<T> {
    fn default() -> Self {
        Self::new()
    }
}

impl<T> Stream for Meltdown<T> {
    type Item = T;

    fn poll_next(
        mut self: Pin<&mut Self>,
        cx: &mut core::task::Context<'_>,
    ) -> core::task::Poll<Option<Self::Item>> {
        Pin::new(&mut self.futures).poll_next(cx)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn can_register_and_run_services() {
        pollster::block_on(async {
            let mut meltdown = Meltdown::new()
                .register(|_| async { "service 1" })
                .register(|_| async { "service 2" })
                .register(|_| async { "service 3" });

            assert!(meltdown.next().await.is_some());
            assert!(meltdown.next().await.is_some());
            assert!(meltdown.next().await.is_some());
            assert!(meltdown.next().await.is_none());
        });
    }

    #[test]
    fn can_trigger_meltdown() {
        pollster::block_on(async {
            let mut meltdown = Meltdown::new()
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

            assert_eq!(meltdown.next().await, Some(1));
            assert_eq!(meltdown.next().await, Some(1));

            meltdown.trigger();

            assert_eq!(meltdown.next().await, Some(2));
            assert_eq!(meltdown.next().await, Some(2));

            assert!(meltdown.next().await.is_none());
        });
    }
}
