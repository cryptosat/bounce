use bounce::bounce_satellite_server::{BounceSatellite, BounceSatelliteServer};
use bounce::{BounceRequest, BounceResponse, Cubesat};
use bounce::{CubesatRequest, CubesatResponse};
use tokio::sync::{broadcast, mpsc};
use tonic::{transport::Server, Request, Response, Status};

pub struct CommsHub {
    cubesats: Vec<Cubesat>,

    broadcast_tx: broadcast::Sender<CubesatRequest>,
    // A broadcast channel that will be shared among the cubesats
    // A mpsc channel to send back either precommit / noncommit back to the ground station
}

impl CommsHub {
    pub fn new() -> CommsHub {
        // TODO: Set appropriate values for the bounds. They're arbitrary values at this point.
        let (broadcast_tx, _) = broadcast::channel(100);

        let mut cubesats = Vec::new();
        // TODO: Provide constructors with the number of cubesats to have.
        let num_cubesats = 10;
        for _ in 0..num_cubesats {
            let broadcast_tx = broadcast_tx.clone();
            let broadcast_rx = broadcast_tx.subscribe();
            let mut c = Cubesat::new(broadcast_rx);

            tokio::spawn(async move {
                c.run().await;
            });
        }

        Self {
            cubesats,
            broadcast_tx,
        }
    }
}

#[tonic::async_trait]
impl BounceSatellite for CommsHub {
    // The bounce function is marked async, so whenever this function is called, we should broadcast
    // the message to sign to cubesats.
    // broadcast channel here: CommsHub -> Cubesats, each cubesat needs to see messages in order
    //  without any loss.
    //
    // Whenever the cubesat receive such request, then the cubesat signs and then sends back
    // the signature (either aggregated or single) to the comms hub.
    // Multi-producer, single consumer channel here, cubesats to commshub
    //  and the comms hub needs to check whether the signature is aggregated, if it is aggregated
    //  then it needs to send it back to the ground station.

    // Sending back can also be managed by a separate thread.
    async fn bounce(
        &self,
        request: Request<BounceRequest>,
    ) -> Result<Response<BounceResponse>, Status> {
        println!("Got a request: {:?}", request);

        let (result_tx, mut result_rx) = mpsc::channel(100);

        let mut cubesat_request = CubesatRequest {
            msg: request.into_inner().message,
            signatures: Vec::new(),
            public_keys: Vec::new(),
            result_tx: result_tx.clone(),
        };

        // Not sure what kind of error handling needs to happen here.
        self.broadcast_tx.send(cubesat_request).unwrap();

        // After broadcastingd the request, now the communications hub will wait for 10 seconds.
        // If the cubesats don't produce either precommit or non commit within that time frame,
        // it will just return non-commit.

        // loop {
        //     tokio::select! {
        //         maybe_response = self.result_rx.recv() => {
        //             if maybe_response.is_err() {
        //                 println!("Received error, exiting...");
        //                 return Err(std::result::Error::);
        //             }

        //             let cubesat_response = maybe_response.unwrap();

        //             let bounce_response = BounceResponse {
        //                 aggregate_public_key: cubesat_response.public_key,
        //                 aggregate_signature: cubesat_response.signature,
        //             }

        //             return Ok(Response::new(cubesat_response));
        //         }
        //     }
        // }

        panic!("unimplemented yet");
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let addr = "[::1]:50051".parse()?;
    let comms_hub = CommsHub::new();

    // This installs a BounceSatelliteServer service.
    Server::builder()
        .add_service(BounceSatelliteServer::new(comms_hub))
        .serve(addr)
        .await?;

    Ok(())
}
