use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use bounce::bounce_satellite_server::{BounceSatellite, BounceSatelliteServer};
use bounce::CubesatRequest;
use bounce::{BounceRequest, BounceResponse, Cubesat};
use tokio::sync::{broadcast, mpsc};
use tonic::{transport::Server, Request, Response, Status};

pub struct CommsHub {
    broadcast_tx: broadcast::Sender<CubesatRequest>,
    // A broadcast channel that will be shared among the cubesats
    // A mpsc channel to send back either precommit / noncommit back to the ground station
}

impl CommsHub {
    // TODO: Define a constructor to parameterize the number of cubesats
    pub fn new() -> CommsHub {
        // TODO: Set appropriate values for the bounds. They're arbitrary values at this point.
        let (broadcast_tx, _) = broadcast::channel(1000);

        let num_cubesats = 10;
        for _ in 0..num_cubesats {
            let broadcast_rx = broadcast_tx.subscribe();
            let mut c = Cubesat::new(broadcast_rx);

            tokio::spawn(async move {
                c.run().await;
            });
        }

        Self { broadcast_tx }
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

        let cubesat_request = CubesatRequest {
            msg: request.into_inner().message,
            signatures: Vec::new(),
            public_keys: Vec::new(),
            result_tx: result_tx.clone(),
        };

        let mut signatures = Vec::new();
        let mut public_keys = Vec::new();

        // Not sure what kind of error handling needs to happen here.
        self.broadcast_tx.send(cubesat_request).unwrap();

        // After broadcastingd the request, now the communications hub will wait for 10 seconds.
        // If the cubesats don't produce either precommit or non commit within that time frame,
        // it will just return non-commit.

        loop {
            tokio::select! {
                Some(cubesat_response) = result_rx.recv() => {

                    signatures.push(cubesat_response.signature);
                    public_keys.push(cubesat_response.public_key);

                    if signatures.len() >= 7 {
                        result_rx.close();
                        println!("aggregating {} number of signatures", signatures.len());
                        let sig_refs: Vec<&[u8]> = signatures.iter().map(|v| v.as_slice()).collect();
                        let aggregate_signature = Bn256.aggregate_signatures(&sig_refs).unwrap();

                        let public_key_refs: Vec<&[u8]> = public_keys.iter().map(|v| v.as_slice()).collect();
                        let aggregate_public_key = Bn256.aggregate_public_keys(&public_key_refs).unwrap();

                        let bounce_response = BounceResponse {
                            aggregate_public_key,
                            aggregate_signature,
                        };
                        println!("returning aggregate signature and public key");


                        return Ok(Response::new(bounce_response));
                    }
                }
            }
        }
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
