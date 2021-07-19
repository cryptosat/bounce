#![warn(rust_2018_idioms)]

use std::error::Error;
use std::net::SocketAddr;
use std::sync::Arc;
use std::time::Duration;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let client_addr = "127.0.0.1:50051";
    let client_addr = client_addr
        .parse::<SocketAddr>()
        .expect("Cannot parse the client address");

    let listen_addr = "127.0.0.1:5005";
    let listen_addr = listen_addr
        .parse::<SocketAddr>()
        .expect("Cannot parse the listening address.");

    let listener = TcpListener::bind(&listen_addr).await?;
    println!("Listening on: {}", listen_addr);

    let cnt: Arc<Mutex<i32>> = Arc::new(Mutex::new(0));
    let cnt3 = Arc::clone(&cnt);

    let mut interval = time::interval(Duration::from_secs(1));
    tokio::spawn(async move {
        loop {
            interval.tick().await;
            if let Ok(mut stream) = TcpStream::connect(client_addr).await {
                let val = *cnt3.lock().await;
                // println!("sending value: {}", val);
                stream
                    .write_all(&val.to_be_bytes())
                    .await
                    .expect("failed to write data to the stream");
            }
        }
    });

    // Loop to listen from the client and increment counter whenever it receives something from the
    // client.
    loop {
        let (mut socket, _) = listener.accept().await?;
        let cnt2 = Arc::clone(&cnt);

        tokio::spawn(async move {
            let mut buf = vec![0; 1024];

            let mut received = false;
            loop {
                let n = socket
                    .read(&mut buf)
                    .await
                    .expect("failed to read data from socket");
                if n == 0 {
                    // n == 0 means the remote side has closed the connection.
                    break;
                } else {
                    // we don't break out of the loop here as the clinet may send more data.
                    received = true;
                }
            }

            if received {
                let mut lock = cnt2.lock().await;
                *lock += 1;
                // println!("incremented cnt to {}", *lock);
            }
        });
    }
}
