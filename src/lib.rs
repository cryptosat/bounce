extern crate bls_signatures_rs;
extern crate chrono;
extern crate tokio;
extern crate tonic;

pub use cubesat::*;
pub mod cubesat;

use tokio::sync::mpsc;

tonic::include_proto!("bounce"); // The string specified here must match the proto package name

#[derive(Clone, Debug)]
pub struct CubesatRequest {
    pub msg: Vec<u8>,
    pub signatures: Vec<Vec<u8>>,
    pub public_keys: Vec<Vec<u8>>,
    pub result_tx: mpsc::Sender<CubesatResponse>,
}

#[derive(Clone, Debug)]
pub struct CubesatResponse {
    pub signature: Vec<u8>,
    pub public_key: Vec<u8>,
}
