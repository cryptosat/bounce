use bls_signatures::{PublicKey, Serialize, Signature};
use bounce::signer_client::SignerClient;
use bounce::SignRequest;

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SignerClient::connect("http://[::1]:50051").await?;

    let msg = chrono::Utc::now().to_rfc2822();
    println!("Message to send: {}", msg);

    let request = tonic::Request::new(SignRequest {
        message: msg.as_bytes().to_vec(),
    });

    let response = client.sign(request).await?.into_inner();

    let sig = Signature::from_bytes(&response.signature)?;
    let pubkey = PublicKey::from_bytes(&response.pubkey)?;

    assert!(pubkey.verify(sig, msg.as_bytes()));
    println!("Verified the message was signed by the cubesat.");
    Ok(())
}
