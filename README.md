# Meltdown

A lightweight service manager to help with graceful shutdown of asynchronous
applications.

## Overview

Meltdown provides a simple way to manage multiple asynchronous services and
coordinate their graceful shutdown. Perfect for web servers, background workers,
or any async application that needs clean shutdown handling.

## Features

- **Lightweight** - Minimal dependencies and simple implementation
- **Runtime Agnostic** - Works with any async runtime (tokio, async-std, smol, etc.)
- **Flexible** - Works with any async service that can handle shutdown signals
- **Simple API** - Easy to use with plain async functions and custom service types

## Quick Start

Add this to your `Cargo.toml`:

```toml
[dependencies]
meltdown = "0.3.0"
```

Basic usage example with `tokio` and `axum`:

```rust
use axum::{routing, Router};
use meltdown::{Meltdown, Token};

async fn api_service(token: Token) -> Result<(), Box<dyn std::error::Error>> {
    let router = Router::new().route("/", routing::get("Hello!"));
    let listener = tokio::net::TcpListener::bind("127.0.0.1:3000").await?;

    axum::serve(listener, router)
        .with_graceful_shutdown(token)
        .await
        .map_err(Into::into)
}

async fn signal_service(token: Token) -> Result<(), Box<dyn std::error::Error>> {
    tokio::select! {
        () = token => Ok(()),
        result = tokio::signal::ctrl_c() => result.map_err(Into::into),
    }
}

#[tokio::main]
async fn main() {
    let mut meltdown = Meltdown::new()
        .register(api_service)
        .register(signal_service);

    // Trigger shutdown when first service completes
    if let Some(result) = meltdown.next().await {
        println!("Got {result:?}, shutting down");
        meltdown.trigger();
    }

    // Wait for remaining services
    while let Some(result) = meltdown.next().await {
        println!("Got {result:?}");
    }
}
```

For detailed usage and examples, check out [the documentation](https://docs.rs/meltdown).

## License

Licensed under the [0BSD](LICENSE) license.
