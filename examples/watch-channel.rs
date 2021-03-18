extern crate tokio;

use tokio::sync::{mpsc, watch};

pub struct Receiver {
    id: usize,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let num_workers = 10;
    let (mpsc_tx, mut mpsc_rx) = mpsc::channel(num_workers);
    let (watch_tx, watch_rx) = watch::channel("hello");

    for i in 0..num_workers {
        let receiver = Receiver { id: i };
        let mut watch_rx = watch_rx.clone();
        let mpsc_tx = mpsc_tx.clone();

        tokio::spawn(async move {
            while watch_rx.changed().await.is_ok() {
                println!("{}, received = {:?}", receiver.id, *watch_rx.borrow());
                if mpsc_tx.send(receiver.id).await.is_err() {
                    println!("receiver dropped");
                    break;
                }
            }
        });
    }

    watch_tx.send("world")?;

    while let Some(id) = mpsc_rx.recv().await {
        println!("Received from {}", id);
    }

    Ok(())
}
