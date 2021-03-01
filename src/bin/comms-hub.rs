use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use bounce::satellite_server::{Satellite, SatelliteServer};
use bounce::{AggregateSignature, BounceRequest, BounceResponse, Cubesat};
use tonic::{transport::Server, Request, Response, Status};

#[derive(Default)]
pub struct CommsHub {
    cubesats: Vec<Cubesat>,
}

impl CommsHub {
    pub fn new() -> CommsHub {
        let mut cubesats = Vec::new();
        // TODO: Provide constructors with the number of cubesats to have.
        let num_cubesats = 10;
        for _ in 0..num_cubesats {
            cubesats.push(Cubesat::new());
        }

        Self {
            cubesats,
            ..Default::default()
        }
    }
}

#[tonic::async_trait]
impl Satellite for CommsHub {
    async fn bounce(
        &self,
        request: Request<BounceRequest>,
    ) -> Result<Response<BounceResponse>, Status> {
        println!("Got a request: {:?}", request);

        let mut signatures = AggregateSignature::new();
        signatures.msg = request.into_inner().message;

        for c in &self.cubesats {
            c.sign(&mut signatures);
        }

        assert_eq!(signatures.public_keys.len(), self.cubesats.len());
        assert_eq!(signatures.signatures.len(), signatures.public_keys.len());

        let sig_refs: Vec<&[u8]> = signatures.signatures.iter().map(|v| v.as_slice()).collect();
        let aggregate_signature = Bn256.aggregate_signatures(&sig_refs).unwrap();

        let public_key_refs: Vec<&[u8]> = signatures
            .public_keys
            .iter()
            .map(|v| v.as_slice())
            .collect();
        let aggregate_public_key = Bn256.aggregate_public_keys(&public_key_refs).unwrap();

        let response = BounceResponse {
            aggregate_public_key,
            aggregate_signature,
        };

        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let comms_hub = CommsHub::new();

    Server::builder()
        .add_service(SatelliteServer::new(comms_hub))
        .serve(addr)
        .await?;

    Ok(())
}
