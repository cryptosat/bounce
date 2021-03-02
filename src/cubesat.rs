use super::{CubesatRequest, CubesatResponse};
use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use rand::{thread_rng, Rng};
use tokio::sync::broadcast;

pub struct Cubesat {
    private_key: Vec<u8>,
    public_key: Vec<u8>,

    // A channel to receive request from the communication hub
    broadcast_rx: broadcast::Receiver<CubesatRequest>,
    // A channel to send signed message back to the communication hub
}

impl Cubesat {
    pub fn new(broadcast_rx: broadcast::Receiver<CubesatRequest>) -> Self {
        let mut rng = thread_rng();

        // generate public and private key pairs.
        let private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let public_key = Bn256.derive_public_key(&private_key).unwrap();

        println!("Cubesat with public_key: {:?}", public_key);

        Self {
            private_key,
            public_key,
            broadcast_rx,
        }
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                maybe_request = self.broadcast_rx.recv() => {
                    if maybe_request.is_err() {
                        println!("Received error, exiting...");
                        break;
                    }

                    let request = maybe_request.unwrap();

                    let signature = Bn256.sign(&self.private_key, &request.msg).unwrap();
                    let _ = Bn256
                        .verify(&signature, &request.msg, &self.public_key)
                        .unwrap();
                    println!("Successfully signed the message {:?}", &request.msg);

                    let response = if request.signatures.len() + 1 >= 7 {
                        let mut sig_refs: Vec<&[u8]> =
                            request.signatures.iter().map(|v| v.as_slice()).collect();
                        sig_refs.push(&signature);
                        let aggregate_signature = Bn256.aggregate_signatures(&sig_refs).unwrap();

                        let mut public_key_refs: Vec<&[u8]> =
                            request.public_keys.iter().map(|v| v.as_slice()).collect();
                        public_key_refs.push(&self.public_key);
                        let aggregate_public_key = Bn256.aggregate_public_keys(&public_key_refs).unwrap();

                        CubesatResponse {
                            aggregated: true,
                            signature: aggregate_signature,
                            public_key: aggregate_public_key,
                        }
                    } else {
                        CubesatResponse {
                            aggregated: false,
                            signature,
                            public_key: self.public_key.clone(),
                        }
                    };

                    // Send the response back to the comms hub.
                }
            }
        }
    }

    pub fn sign(&self, request: &CubesatRequest) -> CubesatResponse {
        let signature = Bn256.sign(&self.private_key, &request.msg).unwrap();
        let _ = Bn256
            .verify(&signature, &request.msg, &self.public_key)
            .unwrap();
        println!("Successfully signed the message {:?}", &request.msg);

        // TODO: use a system wide parameter for the number of cubesats and supermajority
        let supermajority = 7;
        if request.signatures.len() + 1 >= supermajority {
            let mut sig_refs: Vec<&[u8]> =
                request.signatures.iter().map(|v| v.as_slice()).collect();
            sig_refs.push(&signature);
            let aggregate_signature = Bn256.aggregate_signatures(&sig_refs).unwrap();

            let mut public_key_refs: Vec<&[u8]> =
                request.public_keys.iter().map(|v| v.as_slice()).collect();
            public_key_refs.push(&self.public_key);
            let aggregate_public_key = Bn256.aggregate_public_keys(&public_key_refs).unwrap();

            CubesatResponse {
                aggregated: true,
                signature: aggregate_signature,
                public_key: aggregate_public_key,
            }
        } else {
            CubesatResponse {
                aggregated: false,
                signature,
                public_key: self.public_key.clone(),
            }
        }
    }
}
