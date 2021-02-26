extern crate chrono;
extern crate tokio;
extern crate tonic;

tonic::include_proto!("bounce"); // The string specified here must match the proto package name

use bls_signatures::{PublicKey, Signature};

#[derive(Debug, Default)]
pub struct AggregateSignature {
    msg: Vec<u8>,
    signature: Option<Signature>,
    public_keys: Vec<PublicKey>,
}

impl AggregateSignature {
    pub fn new() -> Self {
        Default::default()
    }
}
