use bls_signatures::{PublicKey, Serialize, Signature};
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

    let aggr_sig = Signature::from_bytes(&response.aggregate_signature)?;
    let public_keys: Vec<PublicKey> = response
        .public_keys
        .into_iter()
        .map(|b| PublicKey::from_bytes(&b).unwrap())
        .collect();

    println!("public keys {:?}", public_keys);

    // TODO: Fix this assert to pass. The crate I'm using doesn't seem to support signing the same message.
    assert!(bls_signatures::verify_messages(
        &aggr_sig,
        &[&msg.as_bytes()],
        &public_keys
    ));

    println!("Verified the message was signed by the cubesat.");
    Ok(())
}
