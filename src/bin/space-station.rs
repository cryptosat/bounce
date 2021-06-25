use bounce::bounce_satellite_server::{BounceSatellite, BounceSatelliteServer};
use bounce::{configure_log, configure_log_to_file, BounceConfig, Commit, Cubesat, Phase};
use clap::{crate_authors, crate_version, App, Arg};
// use bounce::Cubesat;
use log::info;
use std::time::Duration;
use tokio::sync::{broadcast, mpsc, Mutex};
use tokio::time::{interval, interval_at, Instant};
use tonic::{transport::Server, Request, Response, Status};

pub struct CubesatInfo {
    id: u32,
    _handle: tokio::task::JoinHandle<()>,
    request_tx: mpsc::Sender<Commit>,
}

pub struct SpaceStation {
    // A channel to receive responses from Cubesats
    result_rx: Mutex<mpsc::Receiver<Commit>>,

    last_slot: Mutex<u32>,

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

impl SpaceStation {
    pub fn new(num_cubesats: u32, timer_tx: &broadcast::Sender<Phase>) -> SpaceStation {
        let (result_tx, result_rx) = mpsc::channel(25);

        let result_rx = Mutex::new(result_rx);

        let mut cubesat_infos = Vec::new();

        for id in 0..num_cubesats {
            let timer_rx = timer_tx.subscribe();
            let (request_tx, request_rx) = mpsc::channel(25);

            let result_tx = result_tx.clone();
            let handle = tokio::spawn(async move {
                let mut cubesat =
                    Cubesat::new(id as usize, num_cubesats, result_tx, request_rx, timer_rx);
                cubesat.run().await;
            });

            cubesat_infos.push(CubesatInfo {
                id,
                _handle: handle,
                request_tx,
            });
        }

        let last_slot = Mutex::new(0);

        Self {
            result_rx,
            last_slot,
            cubesat_infos,
        }
    }
}

#[tonic::async_trait]
impl BounceSatellite for SpaceStation {
    // The bounce function is marked async, so whenever this function is called, we should broadcast
    // the message to sign to cubesats.
    // broadcast channel here: SpaceStation -> Cubesats, each cubesat needs to see messages in order
    //  without any loss.
    //
    // Whenever the cubesat receive such request, then the cubesat signs and then sends back
    // the signature (either aggregated or single) to the comms hub.
    // Multi-producer, single consumer channel here, cubesats to SpaceStation
    //  and the comms hub needs to check whether the signature is aggregated, if it is aggregated
    //  then it needs to send it back to the ground station.

    // Sending back can also be managed by a separate thread.
    async fn bounce(&self, request: Request<Commit>) -> Result<Response<Commit>, Status> {
        info!("Space Station\tReceived a request: {:?}", request);

        let commit: Commit = request.into_inner();

        for cubesat_info in &self.cubesat_infos {
            if cubesat_info.request_tx.send(commit.clone()).await.is_err() {
                info!(
                    "Space Station\tFailed to send a request to Bounce Unit {}",
                    cubesat_info.id
                );
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
                        info!(
                            "Space Station\tReceived an aggregated signature from Bounce Unit {}",
                            precommit.signer_id
                        );
                        // TODO: Change this to use SlotInfo instead of this variable. It turns
                        // out that this information has to be kept somewhere in the
                        // space station too, in addition to among cubesats.
                        let mut idx = self.last_slot.lock().await;
                        if *idx < precommit.i {
                            *idx = precommit.i;
                            return Ok(Response::new(precommit));
                        }
                    } else {
                        info!(
                            "Space Station\tReceived a single signature from Bounce Unit {}",
                            precommit.signer_id
                        );
                        // TODO: Do not send to the cubesat that has sent this precommit.
                        for cubesat_info in &self.cubesat_infos {
                            if cubesat_info.request_tx.send(commit.clone()).await.is_err() {
                                info!(
                                    "Space Station\tFailed to send a request to Bounce Unit {}",
                                    cubesat_info.id
                                );
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
        .arg(
            Arg::with_name("log-to-stdout")
                .long("log-to-stdout")
                .help("By default logs are saved to files, if set log only to stdout.")
                .takes_value(false),
        )
        .arg(
            Arg::with_name("log-dir")
                .long("log-dir")
                .short("l")
                .value_name("LOG_DIR")
                .help("Specify a directory to save logs.")
                .default_value("log"),
        )
        .get_matches();

    let addr = matches.value_of("addr").unwrap();
    let port = matches.value_of("port").unwrap();
    let log_to_stdout = matches.is_present("log-to-stdout");

    if log_to_stdout {
        configure_log()?;
    } else {
        let log_dir = matches.value_of("log-dir").unwrap();
        configure_log_to_file(log_dir, "space-station")?;
    }

    let socket_addr = format!("{}:{}", addr, port).parse()?;

    let bounce_config = BounceConfig {
        num_cubesats: 5,
        slot_duration: 10,
        phase1_duration: 4,
        phase2_duration: 4,
    };

    // Initialized to Stop
    let (timer_tx, _timer_rx) = broadcast::channel(15);

    let comms_hub = SpaceStation::new(bounce_config.num_cubesats, &timer_tx);

    tokio::spawn(async move {
        timer(timer_tx, bounce_config).await;
    });

    // This installs a BounceSatelliteServer service.
    // Question: could this actually successfully make RPCs over unreliable connections between
    // ISS and the Earth?
    Server::builder()
        .add_service(BounceSatelliteServer::new(comms_hub))
        .serve(socket_addr)
        .await?;

    Ok(())
}
