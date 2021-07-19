use clap::{crate_authors, crate_version, App, Arg};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("Experiment runner for Bounce")
        .version(crate_version!())
        .author(crate_authors!())
        .arg(
            Arg::with_name("addr")
            .short("a").value_name("ADDRESS")
            .help("The address of the ground station to connect to")
            .default_value("0.0.0.0"),
        )
        .arg(
            Arg::with_name("port")
            .short("p")
            .value_name("PORT")
            .help("The port of the ground station to connect to")
            .default_value("50051"),
        ).get_matches();

    let addr = matches.value_of("addr").unwrap();
    let port = matches.value_of("port").unwrap();

    Ok(())
}
