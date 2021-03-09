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

#[derive(Clone, Debug)]
pub struct BounceConfig {
    num_cubesats: usize,
    slot_duration: u64,   // in seconds
    phase1_duration: u64, // in seconds
    phase2_duration: u64, // in seconds
}
