
# faucet

> [`deadqueue::limited::Queue`][1] + [`tokio_util::sync::CancellationToken`][2] = `faucet::Faucet`

 [1]: https://docs.rs/deadqueue/latest/deadqueue/
 [2]: https://docs.rs/tokio-util/latest/tokio_util/sync/struct.CancellationToken.html

`Faucet` is a back-pressured MPMC queue that can be drained after signaling completion.

Once completion is signaled, no more items can be added to the queue
and only the remaining items in the queue can be drained. This property is
useful for ensuring all items that were already queued are processed before
shutting down.

You can freely `clone()` a `Facuet` to easily share it between asynchronous tasks for your producers and consumers. You don't need to wrap `Faucet` in an additional `Arc` since `Faucet` internally uses an [`Arc<deadqueue::limited::Queue<T>>`][1]

## Example

You can clone this repo and run this example with `cargo run --example sigint`.

```rust
use std::error::Error;
use std::time::Duration;
use tokio::time::sleep;
use tokio::{spawn, try_join};
use tokio_util::sync::CancellationToken;
use faucet::Faucet;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let app_cancellation = CancellationToken::new();
    ctrlc::set_handler({
        let cancellation = app_cancellation.clone();
        move || cancellation.cancel()
    })?;

    let faucet = Faucet::new_with_cancellation(5, app_cancellation.clone());

    let producer = spawn({
        let faucet = faucet.clone();
        async move {
            for i in 1.. {
                if faucet.push(i).await.is_break() { break; }
                sleep(Duration::from_millis(100)).await;
            }
        }
    });

    let consumer = spawn({
        let faucet = faucet.clone();
        async move {
            while let Some(i) = faucet.next().await {
                sleep(Duration::from_millis(500)).await;
                let status = if faucet.is_cancelled() { "drain" } else { "got" };
                println!("{status} #{i} ({} items waiting)", faucet.len());
            }
        }
    });

    try_join!(producer, consumer)?;
    println!("done");
    Ok(())
}

```

An example run:

```text
got #1 (4 items waiting)
got #2 (5 items waiting)
^Cdrain #3 (5 items waiting)
drain #4 (4 items waiting)
drain #5 (3 items waiting)
drain #6 (2 items waiting)
drain #7 (1 items waiting)
drain #8 (0 items waiting)
done
```

---

## License

Licensed under either of

* Apache License, Version 2.0
  ([LICENSE-APACHE](LICENSE-APACHE) or http://www.apache.org/licenses/LICENSE-2.0)
* MIT license
  ([LICENSE-MIT](LICENSE-MIT) or http://opensource.org/licenses/MIT)

at your option.

## Contribution

Unless you explicitly state otherwise, any contribution intentionally submitted
for inclusion in the work by you, as defined in the Apache-2.0 license, shall be
dual licensed as above, without any additional terms or conditions.
