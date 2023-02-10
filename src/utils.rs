use crate::{Service, Token};
use futures::{future::CatchUnwind, Future, FutureExt};
use pin_project::pin_project;
use std::{
    panic::{AssertUnwindSafe, UnwindSafe},
    pin::Pin,
    task::{Context, Poll},
};

#[pin_project]
pub struct TaggedFuture<F> {
    tag: &'static str,
    #[pin]
    future: F,
}

impl<F> Future for TaggedFuture<F>
where
    F: Future,
{
    type Output = (&'static str, F::Output);

    fn poll(self: Pin<&mut Self>, cx: &mut Context<'_>) -> Poll<Self::Output> {
        let this = self.project();
        this.future.poll(cx).map(|output| (*this.tag, output))
    }
}

pub struct TaggedService<S> {
    tag: &'static str,
    service: S,
}

impl<S> TaggedService<S> {
    pub const fn new(tag: &'static str, service: S) -> Self {
        Self { tag, service }
    }
}

impl<S> Service for TaggedService<S>
where
    S: Service,
{
    type Future = TaggedFuture<S::Future>;

    fn call(self, token: Token) -> Self::Future {
        TaggedFuture {
            tag: self.tag,
            future: self.service.call(token),
        }
    }
}

pub struct CatchPanicService<S> {
    service: S,
}

impl<S> CatchPanicService<S> {
    pub const fn new(service: S) -> Self {
        Self { service }
    }
}

impl<S> Service for CatchPanicService<S>
where
    S: Service,
{
    type Future = CatchUnwind<AssertUnwindSafe<S::Future>>;

    fn call(self, token: Token) -> Self::Future {
        AssertUnwindSafe(self.service.call(token)).catch_unwind()
    }
}
