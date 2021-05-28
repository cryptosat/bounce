use bounce::bounce_satellite_server::{BounceSatellite, BounceSatelliteServer};
use bounce::{BounceConfig, Command, Commit, Cubesat, Phase};
use clap::{crate_authors, crate_version, App, Arg};
// use bounce::Cubesat;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::{interval, interval_at, Instant};
use tonic::{transport::Server, Request, Response, Status};

pub struct CubesatInfo {
    _handle: tokio::task::JoinHandle<()>,
    request_tx: mpsc::Sender<Commit>,
    _command_tx: mpsc::Sender<Command>,
}

pub struct CubesatFlock {
    // A channel to receive responses from Cubesats
    result_rx: Mutex<mpsc::Receiver<Commit>>,

    cubesat_infos: Vec<CubesatInfo>,
}

async fn timer(timer_tx: broadcast::Sender<Phase>, bounce_config: BounceConfig) {
    let slot_duration = Duration::from_secs(bounce_config.slot_duration as u64);
    let start = Instant::now();
    let phase2_start = start + Duration::from_secs(bounce_config.phase1_duration as u64);
    let phase3_start = phase2_start + Duration::from_secs(bounce_config.phase2_duration as u64);
    let mut slot_ticker = interval(slot_duration);
    let mut phase2_ticker = interval_at(phase2_start, slot_duration);
    let mut phase3_ticker = interval_at(phase3_start, slot_duration);

    timer_tx.send(Phase::First).unwrap();
    loop {
        tokio::select! {
            _ = slot_ticker.tick() => {
                timer_tx.send(Phase::First).unwrap();
            }
            _ = phase2_ticker.tick() => {
                timer_tx.send(Phase::Second).unwrap();
            }
            _ = phase3_ticker.tick() => {
                timer_tx.send(Phase::Third).unwrap();
            }
        }
    }
}

impl CubesatFlock {
    // TODO: Define a constructor to parameterize the number of cubesats
    pub fn new(bounce_config: BounceConfig) -> CubesatFlock {
        let (result_tx, result_rx) = mpsc::channel(25);

        let result_rx = Mutex::new(result_rx);

        let num_cubesats = bounce_config.num_cubesats;
        let mut cubesat_infos = Vec::new();

        // Initialized to Stop
        let (timer_tx, _timer_rx) = broadcast::channel(15);

        for idx in 0..bounce_config.num_cubesats {
            let timer_rx = timer_tx.subscribe();
            let (request_tx, request_rx) = mpsc::channel(25);
            let (command_tx, command_rx) = mpsc::channel(10);

            let result_tx = result_tx.clone();
            let handle = tokio::spawn(async move {
                let mut cubesat = Cubesat::new(
                    idx as usize,
                    num_cubesats,
                    result_tx,
                    request_rx,
                    timer_rx,
                    command_rx,
                );
                cubesat.run().await;
            });

            cubesat_infos.push(CubesatInfo {
                _handle: handle,
                request_tx,
                _command_tx: command_tx,
            });
        }

        tokio::spawn(async move {
            timer(timer_tx, bounce_config).await;
        });

        Self {
            result_rx,
            cubesat_infos,
        }
    }
}

#[tonic::async_trait]
impl BounceSatellite for CubesatFlock {
    // The bounce function is marked async, so whenever this function is called, we should broadcast
    // the message to sign to cubesats.
    // broadcast channel here: CubesatFlock -> Cubesats, each cubesat needs to see messages in order
    //  without any loss.
    //
    // Whenever the cubesat receive such request, then the cubesat signs and then sends back
    // the signature (either aggregated or single) to the comms hub.
    // Multi-producer, single consumer channel here, cubesats to CubesatFlock
    //  and the comms hub needs to check whether the signature is aggregated, if it is aggregated
    //  then it needs to send it back to the ground station.

    // Sending back can also be managed by a separate thread.
    async fn bounce(&self, request: Request<Commit>) -> Result<Response<Commit>, Status> {
        println!("Got a request: {:?}", request);

        let commit: Commit = request.into_inner();

        for cubesat_info in &self.cubesat_infos {
            if cubesat_info.request_tx.send(commit.clone()).await.is_err() {
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
                        println!("received aggregated signature");
                        return Ok(Response::new(precommit));
                    } else {
                        println!("received signature, just broadcast");
                        // TODO: Do not send to the cubesat that has sent this precommit.
                        for cubesat_info in &self.cubesat_infos {
                            if cubesat_info.request_tx.send(commit.clone()).await.is_err() {
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
    let matches = App::new("A flock of Bounce cubesat units")
        .version(crate_version!())
        .author(crate_authors!())
        .arg(
            Arg::with_name("addr")
                .short("a")
                .value_name("ADDRESS")
                .help("Specify an alternate address to use.")
                .default_value("0.0.0.0"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .value_name("PORT")
                .help("Specify an alternate port to use.")
                .default_value("50051"),
        )
        .get_matches();

    let addr = matches.value_of("addr").unwrap();
    let port = matches.value_of("port").unwrap();

    let socket_addr = format!("{}:{}", addr, port).parse()?;

    let bounce_config = BounceConfig {
        num_cubesats: 10,
        slot_duration: 10,
        phase1_duration: 4,
        phase2_duration: 4,
    };

    let comms_hub = CubesatFlock::new(bounce_config);

    // This installs a BounceSatelliteServer service.
    // Question: could this actually successfully make RPCs over unreliable connections between
    // ISS and the Earth?
    Server::builder()
        .add_service(BounceSatelliteServer::new(comms_hub))
        .serve(socket_addr)
        .await?;

    Ok(())
}
