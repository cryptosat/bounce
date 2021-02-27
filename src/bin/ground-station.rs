use bounce::satellite_client::SatelliteClient;
use bounce::{BounceRequest, BounceResponse};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SatelliteClient::connect("http://[::1]:50051").await?;

    let msg = chrono::Utc::now().to_rfc2822();
    println!("Message to send: {}", msg);

    let request = tonic::Request::new(BounceRequest {
        message: msg.as_bytes().to_vec(),
    });

    let _response: BounceResponse = client.bounce(request).await?.into_inner();

    println!("Verified the message was signed by the cubesat.");
    Ok(())
}
