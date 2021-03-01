extern crate bls_signatures_rs;
extern crate chrono;
extern crate tokio;
extern crate tonic;

pub use cubesat::*;
pub mod cubesat;

tonic::include_proto!("bounce"); // The string specified here must match the proto package name
