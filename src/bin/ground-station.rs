use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
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

    let response: BounceResponse = client.bounce(request).await?.into_inner();

    let _ = Bn256
        .verify(
            &response.aggregate_signature,
            &msg.as_bytes(),
            &response.aggregate_public_key,
        )
        .unwrap();

    println!("Verified the message was signed by the cubesat.");
    Ok(())
}
