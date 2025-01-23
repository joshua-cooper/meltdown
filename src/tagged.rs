//! Service for tagging other services with additional data.
//!
//! # Examples
//!
//! ```
//! # pollster::block_on(async {
//! use meltdown::{Meltdown, tagged::Tagged};
//! use std::collections::HashSet;
//!
//! let mut meltdown = Meltdown::new()
//!     .register(Tagged::new("foo", |token| token))
//!     .register(Tagged::new("bar", |token| token))
//!     .register(Tagged::new("baz", |token| token));
//!
//! meltdown.trigger();
//!
//! // Collect all responses since the order isn't guaranteed.
//! let mut responses = HashSet::new();
//! responses.insert(meltdown.next().await);
//! responses.insert(meltdown.next().await);
//! responses.insert(meltdown.next().await);
//!
//! let expected = HashSet::from([
//!     Some(("foo", ())),
//!     Some(("bar", ())),
//!     Some(("baz", ())),
//! ]);
//!
//! assert_eq!(responses, expected);
//! # })
//! ```

use crate::Service;
use core::{
    future::Future,
    pin::Pin,
    task::{ready, Context, Poll},
};
use pin_project_lite::pin_project;

/// Tags the underlying service with additional data.
#[derive(Debug, Clone, Copy)]
pub struct Tagged<T, S> {
    tag: T,
    inner: S,
}

pin_project! {
    /// Future for the [`Tagged`] service.
    #[derive(Debug)]
    #[must_use = "futures do nothing unless you `.await` or poll them"]
    pub struct TaggedFuture<T, F> {
        tag: Option<T>,
        #[pin]
        future: F,
    }
}

impl<T, S> Tagged<T, S> {
    /// Creates a new tagged service.
    pub const fn new(tag: T, inner: S) -> Self {
        Self { tag, inner }
    }
}

impl<T, S: Service> Service for Tagged<T, S> {
    type Future = TaggedFuture<T, S::Future>;

    fn run(self, token: crate::Token) -> Self::Future {
        TaggedFuture {
            tag: Some(self.tag),
            future: self.inner.run(token),
        }
    }
}

impl<T, F: Future> Future for TaggedFuture<T, F> {
    type Output = (T, F::Output);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        let output = ready!(this.future.poll(cx));
        let tag = this
            .tag
            .take()
            .expect("this future should never be polled again after resolving");
        Poll::Ready((tag, output))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Token;

    #[test]
    fn response_contains_the_tag() {
        pollster::block_on(async {
            let service = Tagged::new("my-tag", |_token| async { "response" });
            assert_eq!(service.run(Token::new()).await, ("my-tag", "response"));
        });
    }
}
