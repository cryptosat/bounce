use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use bounce::bounce_satellite_client::BounceSatelliteClient;
use bounce::{commit::CommitType, Commit};
use rand::{thread_rng, Rng};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = BounceSatelliteClient::connect("http://[::1]:50051").await?;

    let msg = chrono::Utc::now().to_rfc2822();
    println!("Message to send: {}", msg);

    let mut rng = thread_rng();
    let ground_station_private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    let ground_station_public_key = Bn256
        .derive_public_key(&ground_station_private_key)
        .unwrap();
    let signature = Bn256
        .sign(&ground_station_private_key, &msg.as_bytes())
        .unwrap();

    let precommit = Commit {
        typ: CommitType::Precommit.into(),
        i: 1,
        j: 0,
        msg: msg.as_bytes().to_vec(),
        public_key: ground_station_public_key,
        signature,
        aggregated: false,
    };

    let request = tonic::Request::new(precommit);

    let response = client.bounce(request).await?.into_inner();

    let _ = Bn256
        .verify(&response.signature, &msg.as_bytes(), &response.public_key)
        .unwrap();

    println!("Verified the message was signed by the cubesat.");
    Ok(())
}
