use bounce::satellite_server::{Satellite, SatelliteServer};
use bounce::{BounceRequest, BounceResponse};
use tonic::{transport::Server, Request, Response, Status};

#[derive(Debug, Default)]
pub struct CommsHub {}

impl CommsHub {
    pub fn new() -> CommsHub {
        Default::default()
    }
}

#[tonic::async_trait]
impl Satellite for CommsHub {
    async fn bounce(
        &self,
        request: Request<BounceRequest>,
    ) -> Result<Response<BounceResponse>, Status> {
        println!("Got a request: {:?}", request);

        let response = BounceResponse::default();

        Ok(Response::new(response))
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let comms_hub = CommsHub::default();

    Server::builder()
        .add_service(SatelliteServer::new(comms_hub))
        .serve(addr)
        .await?;

    Ok(())
}
