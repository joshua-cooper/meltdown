//! Service for catching panics.
//!
//! # Examples
//!
//! ```
//! # pollster::block_on(async {
//! use meltdown::{Meltdown, catch_panic::CatchPanic};
//!
//! let mut meltdown = Meltdown::new()
//!     .register(CatchPanic::new(|token| async {
//!         token.await;
//!         "foo"
//!     }))
//!     .register(CatchPanic::new(|_token| async {
//!         panic!("broken!");
//!     }));
//!
//! // Panic is caught and returned as an error.
//! assert!(meltdown.next().await.transpose().is_err());
//!
//! meltdown.trigger();
//!
//! // No interruption for the remaining services.
//! assert_eq!(meltdown.next().await.transpose().unwrap(), Some("foo"));
//! # })
//! ```

use crate::Service;
use alloc::boxed::Box;
use core::{
    any::Any,
    future::Future,
    panic::AssertUnwindSafe,
    pin::Pin,
    task::{Context, Poll},
};
use pin_project_lite::pin_project;

/// Catches panics from the underlying service.
#[derive(Debug, Clone, Copy)]
pub struct CatchPanic<S> {
    inner: S,
}

pin_project! {
    /// Future for the [`CatchPanic`] service.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct CatchPanicFuture<F> {
        #[pin]
        future: F,
    }
}

impl<S> CatchPanic<S> {
    /// Creates a new panic catching service.
    pub const fn new(inner: S) -> Self {
        Self { inner }
    }
}

impl<S: Service> Service for CatchPanic<S> {
    type Future = CatchPanicFuture<S::Future>;

    fn run(self, token: crate::Token) -> Self::Future {
        CatchPanicFuture {
            future: self.inner.run(token),
        }
    }
}

impl<F: Future> Future for CatchPanicFuture<F> {
    type Output = Result<F::Output, Box<dyn Any + Send>>;

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        match std::panic::catch_unwind(AssertUnwindSafe(|| this.future.poll(cx))) {
            Ok(poll) => poll.map(Ok),
            Err(panic) => Poll::Ready(Err(panic)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Token;

    async fn panic_service(_token: Token) {
        panic!();
    }

    #[test]
    fn can_catch_panics() {
        pollster::block_on(async {
            let service = CatchPanic::new(panic_service);
            assert!(service.run(Token::new()).await.is_err());
        });
    }
}
