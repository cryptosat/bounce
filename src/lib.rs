extern crate chrono;
extern crate tokio;
extern crate tonic;

pub use cubesat::*;
pub mod cubesat;

tonic::include_proto!("bounce"); // The string specified here must match the proto package name

use bls_signatures::{PublicKey, Signature};

#[derive(Debug, Default)]
pub struct AggregateSignature {
    pub msg: Vec<u8>,
    pub signatures: Vec<Signature>,
    pub public_keys: Vec<PublicKey>,
}

impl AggregateSignature {
    pub fn new() -> Self {
        Default::default()
    }
}
