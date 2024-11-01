use crate::Token;
use core::future::Future;

/// A long running service that supports graceful shutdown.
pub trait Service {
    /// The future return value.
    type Future: Future;

    /// Runs this service.
    ///
    /// When the provided token is triggered, this service will begin a graceful shutdown.
    fn run(self, token: Token) -> Self::Future;
}

impl<F, Fut> Service for F
where
    F: FnOnce(Token) -> Fut,
    Fut: Future + Send + 'static,
{
    type Future = Fut;

    fn run(self, token: Token) -> Self::Future {
        self(token)
    }
}
