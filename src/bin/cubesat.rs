use bls_signatures::{PrivateKey, PublicKey, Serialize};
use bounce::signer_server::{Signer, SignerServer};
use bounce::{SignRequest, SignResponse};
use rand::thread_rng;
use tonic::{transport::Server, Request, Response, Status};

#[derive(Debug)]
pub struct Cubesat {
    private_key: PrivateKey,
    public_key: PublicKey,
}

impl Default for Cubesat {
    fn default() -> Self {
        let mut rng = thread_rng();

        let private_key = PrivateKey::generate(&mut rng);
        let public_key = private_key.public_key();
        // generate public and private key pairs.

        Cubesat {
            private_key,
            public_key,
        }
    }
}

impl Cubesat {
    pub fn new() -> Cubesat {
        Default::default()
    }
}

#[tonic::async_trait]
impl Signer for Cubesat {
    async fn sign(&self, request: Request<SignRequest>) -> Result<Response<SignResponse>, Status> {
        println!("Got a request: {:?}", request);

        let sig = self.private_key.sign(&request.into_inner().message);
        let response = SignResponse {
            pubkey: self.public_key.as_bytes(),
            signature: sig.as_bytes(),
        };

        Ok(Response::new(response))
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
