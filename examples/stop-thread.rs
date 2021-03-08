use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time;

#[tokio::main]
async fn main() {
    let (tx, mut rx) = mpsc::channel(1);

    tokio::spawn(async move {
        let mut interval = time::interval(Duration::from_secs(1));
        loop {
            tokio::select! {
                _ = rx.recv() => {
                    println!("Received signal to stop.");
                    break;
                }
                _ = interval.tick() => {
                    println!("1 seconds elapsed.");
                }
            }
        }
    });

    time::sleep(Duration::from_secs(5)).await;

    tx.send(()).await.unwrap();
}
