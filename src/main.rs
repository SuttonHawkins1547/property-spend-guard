mod property_guard;

use property_guard::{process_property_task, InfraiClient, MaintenanceKind, PropertyTask};

fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
    use std::future::Future;
    use std::pin::Pin;
    use std::sync::Arc;
    use std::task::{Context, Poll, Wake, Waker};

    struct Noop;
    impl Wake for Noop {
        fn wake(self: Arc<Self>) {}
    }
    let waker = Waker::from(Arc::new(Noop));
    let mut context = Context::from_waker(&waker);
    let mut future = Pin::from(Box::new(future));
    loop {
        match Future::poll(future.as_mut(), &mut context) {
            Poll::Ready(value) => return value,
            Poll::Pending => std::thread::yield_now(),
        }
    }
}

fn main() {
    let client = match InfraiClient::from_env() {
        Ok(client) => client,
        Err(error) => {
            eprintln!("configuration error: {error:?}");
            std::process::exit(1);
        }
    };
    let task = PropertyTask {
        kind: MaintenanceKind::Request,
        property_name: "Harbor Court 4B".to_owned(),
        detail: "Resident reports a leaking kitchen faucet.".to_owned(),
    };
    let result = block_on(process_property_task(&client, 75.0, &task));
    match result {
        Ok(response) => println!("property decision: {response}"),
        Err(error) => {
            eprintln!("request rejected: {error:?}");
            std::process::exit(1);
        }
    }
}
