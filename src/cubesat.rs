extern crate bitcoin_hashes;
extern crate secp256k1;
extern crate tokio;
extern crate tonic;

use secp256k1::Secp256k1;

use bitcoin_hashes::{sha256, Hash};
use secp256k1::rand::rngs::OsRng;
// use secp256k1::{
//     Error, Message, PublicKey, Secp256k1, SecretKey, Signature, Signing, Verification,
// };
use tonic::{transport::Server, Request, Response, Status};

use bounce::signer_server::{Signer, SignerServer};
use bounce::{SignRequest, SignResponse};
pub mod bounce {
    tonic::include_proto!("bounce"); // The string specified here must match the proto package name
}

#[derive(Debug)]
pub struct Cubesat {
    secp: Secp256k1<secp256k1::SignOnly>,
    seckey: secp256k1::SecretKey,
    pubkey: secp256k1::PublicKey,
}

impl Default for Cubesat {
    fn default() -> Self {
        let secp = Secp256k1::new();
        let mut rng = OsRng::new().unwrap();
        // generate public and private key pairs.
        let (seckey, pubkey) = secp.generate_keypair(&mut rng);

        Cubesat {
            secp: Secp256k1::signing_only(),
            seckey,
            pubkey,
        }
    }
}

impl Cubesat {
    pub fn new() -> Cubesat {
        Default::default()
    }

    fn sign(&self, msg: &[u8]) -> Result<secp256k1::Signature, secp256k1::Error> {
        let msg = sha256::Hash::hash(msg);
        let msg = secp256k1::Message::from_slice(&msg)?;
        Ok(self.secp.sign(&msg, &self.seckey))
    }
}

#[tonic::async_trait]
impl Signer for Cubesat {
    async fn sign(&self, request: Request<SignRequest>) -> Result<Response<SignResponse>, Status> {
        println!("Got a request: {:?}", request);

        match self.sign(request.into_inner().message.as_bytes()) {
            Ok(_sig) => Ok(Response::new(bounce::SignResponse {
                signature: "hello".to_string(),
            })),
            Err(_e) => Err(tonic::Status::internal("Failed to sign")),
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let cubesat = Cubesat::default();

    Server::builder()
        .add_service(SignerServer::new(cubesat))
        .serve(addr)
        .await?;

    Ok(())
}

// fn verify<C: Verification>(
//     secp: &Secp256k1<C>,
//     msg: &[u8],
//     sig: Signature,
//     pubkey: PublicKey,
// ) -> Result<bool, Error> {
//     let msg = sha256::Hash::hash(msg);
//     let msg = Message::from_slice(&msg)?;
//     Ok(secp.verify(&msg, &sig, &pubkey).is_ok())
// }

// fn sign<C: Signing>(
//     secp: &Secp256k1<C>,
//     msg: &[u8],
//     seckey: &SecretKey,
// ) -> Result<Signature, Error> {
//     let msg = sha256::Hash::hash(msg);
//     let msg = Message::from_slice(&msg)?;
//     Ok(secp.sign(&msg, &seckey))
// }

// fn main() {
//     let secp = Secp256k1::new();
//     let mut rng = OsRng::new().unwrap();
//     // generate public and private key pairs.
//     let (seckey, pubkey) = secp.generate_keypair(&mut rng);
//     assert_eq!(pubkey, PublicKey::from_secret_key(&secp, &seckey));

//     // Read message to sign
//     println!("Enter your name: ");
//     let mut name = String::new();
//     std::io::stdin()
//         .read_line(&mut name)
//         .expect("Failed to read line");

//     println!("Hello, {}!", &name[..name.len() - 1]);

//     let sig = sign(&secp, name.as_bytes(), &seckey).unwrap();
//     assert!(verify(&secp, name.as_bytes(), sig, pubkey).unwrap());
// }
