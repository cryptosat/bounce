use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use bounce::bounce_satellite_client::BounceSatelliteClient;
use bounce::{commit::CommitType, configure_log, configure_log_to_file, Commit};
use clap::{crate_authors, crate_version, App, Arg};
use tokio::time::interval;
use log::info;
use std::time::Duration;
use rand::{thread_rng, Rng};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("Bounce ground station")
        .version(crate_version!())
        .author(crate_authors!())
        .arg(
            Arg::with_name("addr")
                .short("a")
                .value_name("ADDRESS")
                .help("Specify an alternate address to connect to.")
                .default_value("0.0.0.0"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .value_name("PORT")
                .help("Specify an alternate port to connect to.")
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

    let dst = format!("http://{}:{}", addr, port);

    let mut client = BounceSatelliteClient::connect(dst).await?;


    let slot_duration = Duration::from_secs(10);
    let mut slot_ticker = interval(slot_duration);

    for _ in 0..10 {
        tokio::select!{
            _ = slot_ticker.tick() => {
                let msg = chrono::Utc::now().to_rfc2822();
                info!("Ground Station\tSending message: {}", msg);

                let mut rng = thread_rng();
                let ground_station_private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
                let ground_station_public_key = Bn256
                    .derive_public_key(&ground_station_private_key)
                    .unwrap();
                let signature = Bn256
                    .sign(&ground_station_private_key, &msg.as_bytes())
                    .unwrap();

                let precommit = Commit {
                    typ: CommitType::Precommit.into(),
                    i: 1,
                    j: 0,
                    msg: msg.as_bytes().to_vec(),
                    public_key: ground_station_public_key,
                    signature,
                    aggregated: false,
                    // TODO: FIXME
                    signer_id: 100,
                };

                let request = tonic::Request::new(precommit);

                let start = chrono::Utc::now();

                let response = client.bounce(request).await?.into_inner();

                let end = chrono::Utc::now();

                let _ = Bn256
                    .verify(&response.signature, &msg.as_bytes(), &response.public_key)
                    .unwrap();

                info!(
                    "Ground Station\tVerified that the message was signed by the flock in {} ms.",
                    (end - start).num_milliseconds()
                );
            }
        }
    }

    Ok(())
}
