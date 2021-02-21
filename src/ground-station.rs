extern crate secp256k1;
extern crate tokio;
extern crate tonic;

use bitcoin_hashes::{sha256, Hash};
use bounce::signer_client::SignerClient;
use bounce::SignRequest;
use secp256k1::{Error, Message, PublicKey, Secp256k1, Signature, Verification};

pub mod bounce {
    tonic::include_proto!("bounce"); // The string specified here must match the proto package name
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut client = SignerClient::connect("http://[::1]:50051").await?;

    let msg = "1";

    let request = tonic::Request::new(SignRequest {
        message: msg.as_bytes().to_vec(),
    });

    let response = client.sign(request).await?.into_inner();

    let secp = Secp256k1::verification_only();

    let sig = Signature::from_compact(&response.signature)?;
    let pubkey = PublicKey::from_slice(&response.pubkey)?;

    verify(&secp, msg.as_bytes(), sig, pubkey)?;
    Ok(())
}

fn verify<C: Verification>(
    secp: &Secp256k1<C>,
    msg: &[u8],
    sig: Signature,
    pubkey: PublicKey,
) -> Result<bool, Error> {
    let msg = sha256::Hash::hash(msg);
    let msg = Message::from_slice(&msg)?;
    Ok(secp.verify(&msg, &sig, &pubkey).is_ok())
}
