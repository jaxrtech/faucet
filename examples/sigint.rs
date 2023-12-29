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
