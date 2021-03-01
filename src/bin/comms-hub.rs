use bounce::bounce_satellite_server::{BounceSatellite, BounceSatelliteServer};
use bounce::{BounceRequest, BounceResponse, Cubesat, CubesatRequest};
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
impl BounceSatellite for CommsHub {
    async fn bounce(
        &self,
        request: Request<BounceRequest>,
    ) -> Result<Response<BounceResponse>, Status> {
        println!("Got a request: {:?}", request);

        let mut cubesat_request = CubesatRequest {
            msg: request.into_inner().message,
            signatures: Vec::new(),
            public_keys: Vec::new(),
        };

        for c in &self.cubesats {
            let cubesat_response = c.sign(&cubesat_request);

            if cubesat_response.aggregated {
                let response = BounceResponse {
                    aggregate_public_key: cubesat_response.public_key,
                    aggregate_signature: cubesat_response.signature,
                };

                return Ok(Response::new(response));
            } else {
                cubesat_request.signatures.push(cubesat_response.signature);
                cubesat_request
                    .public_keys
                    .push(cubesat_response.public_key);
            }
        }

        // TODO: this is just to make this file compile, better error handling
        panic!("failed to get aggregate signature from cubesat");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let comms_hub = CommsHub::new();

    Server::builder()
        .add_service(BounceSatelliteServer::new(comms_hub))
        .serve(addr)
        .await?;

    Ok(())
}
