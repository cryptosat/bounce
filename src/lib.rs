extern crate bitcoin_hashes;
extern crate chrono;
extern crate secp256k1;
extern crate tokio;
extern crate tonic;

tonic::include_proto!("bounce"); // The string specified here must match the proto package name
