use super::{CubesatRequest, CubesatResponse};
use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use rand::{thread_rng, Rng};
use std::time::Duration;
use tokio::sync::{broadcast, mpsc};
use tokio::time::{interval, interval_at, Instant};

pub struct Cubesat {
    private_key: Vec<u8>,
    public_key: Vec<u8>,

    // A channel to receive request from the communication hub
    broadcast_rx: broadcast::Receiver<CubesatRequest>,
}

#[derive(Clone, Debug)]
pub enum CommitType {
    Precommit,
    Noncommit,
}

#[derive(Clone, Debug)]
pub struct Commit {
    typ: CommitType,
    id: usize,
    signature: Vec<u8>,
}

#[derive(Clone, Debug)]
pub enum Command {
    Stop,
    // Sign this message sent from ground station
    Sign(Vec<u8>),
    // Aggregate either precommit or noncommit
    Aggregate(Commit),
}

#[derive(Clone, Debug)]
pub struct SlotConfig {
    duration: u64,        // in seconds
    phase1_duration: u64, // in seconds
    phase2_duration: u64, // in seconds
}

pub enum Phase {
    Stop,
    First,
    Second,
    Third,
}

pub struct CubesatWithSlot {
    // Number of cubesats including itself.
    num_cubesats: usize,

    id: usize,
    slot_id: usize,

    // Configuration for slot
    slot_config: SlotConfig,
    phase: Phase,

    // Whether this cubesat has signed a precommit or non-commit for current slot
    signed: bool,
    // Whether this cubesat has aggregated signatures of at least supermajority of num_cubesats
    aggregated: bool,

    public_key: Vec<u8>,
    private_key: Vec<u8>,

    // (id, signature) of precommtis or noncommits received for this slot.
    precommits: Vec<Commit>,
    noncommits: Vec<Commit>,

    // sender to send to communications hub
    result_tx: mpsc::Sender<Command>,
    // receiver to receive commands from the communications hub
    request_rx: mpsc::Receiver<Command>,
}

impl CubesatWithSlot {
    pub fn new(
        num_cubesats: usize,
        id: usize,
        slot_config: SlotConfig,
        result_tx: mpsc::Sender<Command>,
        request_rx: mpsc::Receiver<Command>,
    ) -> Self {
        let mut rng = thread_rng();

        // generate public and private key pairs.
        let private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let public_key = Bn256.derive_public_key(&private_key).unwrap();

        Self {
            num_cubesats,
            id,
            slot_id: 0,
            slot_config,
            phase: Phase::Stop,
            signed: false,
            aggregated: false,
            public_key,
            private_key,
            precommits: Vec::new(),
            noncommits: Vec::new(),
            result_tx,
            request_rx,
        }
    }

    pub async fn run(&mut self) {
        let slot_duration = Duration::from_secs(self.slot_config.duration);
        let mut slot_ticker = interval(slot_duration);
        let start = Instant::now();
        let phase2_start = start + Duration::from_secs(self.slot_config.phase1_duration);
        let phase3_start = phase2_start + Duration::from_secs(self.slot_config.phase2_duration);
        let mut phase2_ticker = interval_at(phase2_start, slot_duration);
        let mut phase3_ticker = interval_at(phase3_start, slot_duration);

        loop {
            tokio::select! {
                _ = slot_ticker.tick() => {
                    // Have to sign and send noncommit for (j + 1, i)

                    self.precommits.clear();
                    self.noncommits.clear();
                    self.phase = Phase::First;
                    self.signed = false;
                    self.aggregated = false;
                    println!("slot timer tick");
                    self.slot_id += 1;
                }
                _ = phase2_ticker.tick() => {
                    self.phase = Phase::Second;
                }
                _ = phase3_ticker.tick() => {
                    self.phase = Phase::Third;
                }
                Some(cmd) = self.rx.recv() => {
                    match cmd {
                        Command::Stop => {
                            self.phase = Phase::Stop;
                            println!("exiting the loop...");
                            break;
                        }
                        Command::Sign(msg) => {
                            if self.signed {
                                // already signed for this slot.
                                return;
                            }

                            let signature = Bn256.sign(&self.private_key, &msg).unwrap();

                            // TODO: check errors
                            self.tx.send(
                                Command::Aggregate(Commit {typ: CommitType::Precommit, id: self.id, signature})
                            ).await.unwrap();
                        }
                        Command::Aggregate(commit) => {
                            match commit.typ {
                                CommitType::Precommit => {
                                    self.precommits.push(commit);
                                }
                                CommitType::Noncommit => {
                                    self.noncommits.push(commit);
                                }
                            }
                        }
                    }
                }
            }
        }
    }
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

                    // Phase 1, before i_commitonly
                    // If received a pre-commit from the ground station, or the other cubesats,
                    // then sign it and broadcast to others
                    // If received a non-commit then ignore

                    // Phase 2. before i_noncommitinitiate
                    // If it hasn't already signed precommit, and if it receives precommit then
                    // signs and broadcast
                    // if it receives non-commit, then signs non-commit and broadcast

                    // Phase 3, before i_end
                    //

                    // Each cubesat needs to keep track of the signatures it has received for current
                    // slot


                    if maybe_request.is_err() {
                        println!("Received error, exiting...");
                        break;
                    }

                    let request = maybe_request.unwrap();

                    if request.public_keys.contains(&self.public_key) {
                        println!("received a message that contains my signature");
                        continue;
                    }

                    let signature = Bn256.sign(&self.private_key, &request.msg).unwrap();
                    let _ = Bn256
                        .verify(&signature, &request.msg, &self.public_key)
                        .unwrap();
                    println!("Successfully signed the message");

                    let response = CubesatResponse {
                        signature,
                        public_key: self.public_key.clone(),
                    };

                    // Send the response back to the comms hub.

                    if request.result_tx.send(response).await.is_err() {
                        println!("result tx closed for this slot");
                    }
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
                signature: aggregate_signature,
                public_key: aggregate_public_key,
            }
        } else {
            CubesatResponse {
                signature,
                public_key: self.public_key.clone(),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn cubesat_stop() {
        let (result_tx, _) = mpsc::channel(1);
        let (request_tx, request_rx) = mpsc::channel(15);

        let mut c = CubesatWithSlot::new(
            1,
            0,
            SlotConfig {
                duration: 10,
                phase1_duration: 4,
                phase2_duration: 4,
            },
            result_tx,
            request_rx,
        );

        tokio::spawn(async move {
            c.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        request_tx.send(Command::Stop)
            .await
            .expect("Failed to send stop command");
    }
}
