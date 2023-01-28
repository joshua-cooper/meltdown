use meltdown::{
    utils::{CatchPanicService, TaggedService},
    Meltdown, Service, Token,
};
use std::time::Duration;

async fn short_sleep(_token: Token) {
    tokio::time::sleep(Duration::from_secs(1)).await;
}

async fn long_sleep(_token: Token) {
    tokio::time::sleep(Duration::from_secs(5)).await;
}

async fn sleep_panic(_token: Token) {
    tokio::time::sleep(Duration::from_secs(1)).await;
    panic!("something broke!");
}

fn service<S>(name: &'static str, service: S) -> CatchPanicService<TaggedService<S>>
where
    S: Service,
{
    // service
    //     .layer(TaggedLayer::new(name))
    //     .layer(CatchPanicLayer::new())
    //     .layer(RestartLayer::new())

    CatchPanicService::new(TaggedService::new(name, service))
}

#[tokio::main]
async fn main() {
    let mut meltdown = Meltdown::new();

    meltdown.register(service("sleep-1", long_sleep));
    meltdown.register(service("sleep-2", short_sleep));
    meltdown.register(service("sleep-3", short_sleep));
    meltdown.register(service("sleep-4", long_sleep));
    meltdown.register(service("sleep-5", sleep_panic));
    meltdown.register(service("sleep-6", short_sleep));

    while let Some(result) = meltdown.wait_next().await {
        println!("{result:?}");
    }
}
