extern crate bls_signatures_rs;
extern crate chrono;
extern crate tokio;
extern crate tonic;

pub use cubesat::*;
pub mod cubesat;

tonic::include_proto!("bounce"); // The string specified here must match the proto package name

#[derive(Debug, Default)]
pub struct AggregateSignature {
    pub msg: Vec<u8>,
    pub signatures: Vec<Vec<u8>>,
    pub public_keys: Vec<Vec<u8>>,
}

impl AggregateSignature {
    pub fn new() -> Self {
        Default::default()
    }
}
