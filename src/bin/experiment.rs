use clap::{crate_authors, crate_version, App, Arg};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let matches = App::new("Experiment runner for Bounce")
        .version(crate_version!())
        .author(crate_authors!())
        .arg(
            Arg::with_name("addr")
                .short("a")
                .value_name("ADDRESS")
                .help("The address of the ground station to connect to")
                .default_value("0.0.0.0"),
        )
        .arg(
            Arg::with_name("port")
                .short("p")
                .value_name("PORT")
                .help("The port of the ground station to connect to")
                .default_value("50051"),
        )
        .get_matches();

    let _addr = matches.value_of("addr").unwrap();
    let _port = matches.value_of("port").unwrap();

    Ok(())
}

// Reads a file which contains Bounce configuration
// Bounce configuration consist of the following
// 1. Ground Station configuration
//  - IP address, port
//  - number of ground stations
// 2. Flock configuration
//  - IP address, port
//  - number of bounce units in the flock
//  - failure modes for each of the bounce units
// 3. Slot configuration
//  - the length of the slot and each phases
// 4. Experiment configuration
//  - logging options (log to stdout or log file directory)

// Start
// Timer server
// space station
// flock
// Wait for above to be ready
// Then start the experiment

// how to represent the configuration
// 1. using a protobuf
// 2. using a json file
// 3. Using a plain text file

// Representation of the experiment configuration
// - Experiment strategy
//  1. random
//  2. predefined with a pattern
//  3. All manual

//

// How to make sure each experiment is consistent over runs.
