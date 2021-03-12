use bounce::bounce_satellite_server::{BounceSatellite, BounceSatelliteServer};
use bounce::{BounceConfig, Command, Commit, Cubesat};
// use bounce::Cubesat;
use tokio::sync::{mpsc, Mutex};
use tonic::{transport::Server, Request, Response, Status};

pub struct CubesatInfo {
    handle: tokio::task::JoinHandle<()>,
    request_tx: mpsc::Sender<Commit>,
    command_tx: mpsc::Sender<Command>,
}

pub struct CommsHub {
    // A channel to receive responses from Cubesats
    result_rx: Mutex<mpsc::Receiver<Commit>>,

    cubesat_infos: Vec<CubesatInfo>,
}

impl CommsHub {
    // TODO: Define a constructor to parameterize the number of cubesats
    pub fn new(bounce_config: BounceConfig) -> CommsHub {
        let (result_tx, result_rx) = mpsc::channel(15);

        let result_rx = Mutex::new(result_rx);

        let mut cubesat_infos = Vec::new();

        for idx in 0..bounce_config.num_cubesats {
            let (request_tx, request_rx) = mpsc::channel(15);
            let (command_tx, command_rx) = mpsc::channel(10);

            let result_tx = result_tx.clone();
            let bounce_config = bounce_config.clone();
            let handle = tokio::spawn(async move {
                let mut cubesat = Cubesat::new(
                    idx as usize,
                    bounce_config,
                    result_tx,
                    request_rx,
                    command_rx,
                );
                cubesat.run().await;
            });

            cubesat_infos.push(CubesatInfo {
                handle,
                request_tx,
                command_tx,
            });
        }

        Self {
            result_rx,
            cubesat_infos,
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
    async fn bounce(&self, request: Request<Commit>) -> Result<Response<Commit>, Status> {
        println!("Got a request: {:?}", request);

        let commit: Commit = request.into_inner();

        for cubesat_info in &self.cubesat_infos {
            if let Err(_) = cubesat_info.request_tx.send(commit.clone()).await {
                println!("failed to send to a cubesat");
            }
        }

        // let mut signatures = Vec::new();
        // let mut public_keys = Vec::new();

        // Not sure what kind of error handling needs to happen here.
        // self.broadcast_tx.send(cubesat_request).unwrap();

        // After broadcastingd the request, now the communications hub will wait for 10 seconds.
        // If the cubesats don't produce either precommit or non commit within that time frame,
        // it will just return non-commit.

        let mut receiver = self.result_rx.lock().await;

        loop {
            match receiver.recv().await {
                Some(precommit) => {
                    if precommit.aggregated {
                        return Ok(Response::new(precommit));
                    } else {
                        for cubesat_info in &self.cubesat_infos {
                            if let Err(_) = cubesat_info.request_tx.send(commit.clone()).await {
                                println!("failed to send to a cubesat");
                            }
                        }
                    }
                }
                _ => {
                    panic!("something didn't work out");
                }
            }
        }
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // TODO: parse program config
    // 1. time to run the experiment, it will shut off after this time.
    // 2. Bounce config
    //  - number of cubesats
    //  - slot duration
    //  - phase 1 duration
    //  - phase 2 duration
    // 3. The IP:PORT to use

    let addr = "[::1]:50051".parse()?;

    let bounce_config = BounceConfig {
        num_cubesats: 10,
        slot_duration: 10,
        phase1_duration: 4,
        phase2_duration: 4,
    };

    let comms_hub = CommsHub::new(bounce_config);

    // This installs a BounceSatelliteServer service.
    // Question: could this actually successfully make RPCs over unreliable connections between
    // ISS and the Earth?
    Server::builder()
        .add_service(BounceSatelliteServer::new(comms_hub))
        .serve(addr)
        .await?;

    Ok(())
}
