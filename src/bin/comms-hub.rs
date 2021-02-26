use bls_signatures::Serialize;
use bounce::satellite_server::{Satellite, SatelliteServer};
use bounce::{AggregateSignature, BounceRequest, BounceResponse, Cubesat};
use tonic::{transport::Server, Request, Response, Status};

#[derive(Debug, Default)]
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

        // TODO: handle error
        let aggregate_signature = bls_signatures::aggregate(&signatures.signatures).unwrap();

        // TODO: Fix this assert to pass. The crate I'm using doesn't seem to support signing the same message.
        assert!(bls_signatures::verify_messages(
            &aggregate_signature,
            &[&signatures.msg],
            &signatures.public_keys
        ));

        let response = BounceResponse {
            public_keys: signatures
                .public_keys
                .into_iter()
                .map(|p| p.as_bytes())
                .collect(),
            aggregate_signature: aggregate_signature.as_bytes(),
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
