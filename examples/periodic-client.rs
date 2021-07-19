#![warn(rust_2018_idioms)]

use std::error::Error;
use std::net::SocketAddr;
use std::time::Duration;
use tokio::io;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpListener;
use tokio::net::TcpStream;
use tokio::time;

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let server_addr = "127.0.0.1.8080";
    let server_addr = server_addr.parse::<SocketAddr>()?;

    // Listen for anything that's sent from the server.
    let listen_addr = "127.0.0.1:50051";
    let listen_addr = listen_addr.parse::<SocketAddr>()?;

    let listener = TcpListener::bind(&listen_addr).await?;
    println!("Listening on: {}", listen_addr);

    // An interval that ticks every 100ms.
    let mut interval = time::interval(Duration::from_millis(100));

    tokio::spawn(async move {
        let mut i: i32 = 0;
        loop {
            interval.tick().await;
            let mut stream = TcpStream::connect(server_addr)
                .await
                .expect("failed to connect to the server");
            i += 1;
            stream
                .write_all(&i.to_be_bytes())
                .await
                .expect("failed to write data to the stream");
        }
    });

    loop {
        let (mut socket, _) = listener.accept().await?;
        tokio::spawn(async move {
            let mut buf = vec![0; 1024];
            loop {
                let n = socket
                    .read(&mut buf)
                    .await
                    .expect("failed to read data from socket");

                if n == 0 {
                    return;
                }
                io::stdout()
                    .write_all(&buf)
                    .await
                    .expect("Failed to write data to stdout");
            }
        });
    }
}
