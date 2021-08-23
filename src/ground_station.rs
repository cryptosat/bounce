use crate::{commit::CommitType, supermajority, BounceRequest, Commit, Phase, SlotInfo};
use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use log::info;
use rand::{thread_rng, Rng};
use tokio::sync::{broadcast, mpsc};

pub enum StationType {
    Sending = 0,
    Listening,
}

pub struct GroundStation {
    pub station_id: u32,
    pub station_type: StationType,

    // Number of all ground stations including this one.
    num_stations: u32,

    public_key: Vec<u8>,
    private_key: Vec<u8>,

    slot_info: SlotInfo,
    // Receiver for phase transitions.
    timer_rx: broadcast::Receiver<Phase>,

    request_rx: mpsc::Receiver<BounceRequest>,

    commit_tx: mpsc::Sender<Commit>,

    commit_rx: broadcast::Receiver<Commit>,
}

impl GroundStation {
    pub fn new(
        station_id: u32,
        station_type: StationType,
        num_stations: u32,
        timer_rx: broadcast::Receiver<Phase>,
        request_rx: mpsc::Receiver<BounceRequest>,
        commit_tx: mpsc::Sender<Commit>,
        commit_rx: broadcast::Receiver<Commit>,
    ) -> GroundStation {
        let mut rng = thread_rng();

        // generate public and private key pairs.
        let private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let public_key = Bn256.derive_public_key(&private_key).unwrap();
        let slot_info = SlotInfo::new();

        GroundStation {
            station_id,
            station_type,
            num_stations,
            public_key,
            private_key,
            slot_info,
            timer_rx,
            request_rx,
            commit_tx,
            commit_rx,
        }
    }

    fn sign(&self, msg: &[u8]) -> Vec<u8> {
        Bn256.sign(&self.private_key, msg).unwrap()
    }

    fn aggregate(&self, bounce_request: &BounceRequest) -> Commit {
        let sig_refs: Vec<&[u8]> = bounce_request
            .signatures
            .iter()
            .map(|sig| sig.as_ref())
            .collect();
        let aggregate_signature = Bn256.aggregate_signatures(&sig_refs).unwrap();

        let public_key_refs: Vec<&[u8]> = bounce_request
            .public_keys
            .iter()
            .map(|key| key.as_ref())
            .collect();
        let aggregate_public_key = Bn256.aggregate_public_keys(&public_key_refs).unwrap();

        Commit {
            typ: CommitType::Precommit.into(),
            i: self.slot_info.i,
            j: self.slot_info.j,
            msg: bounce_request.msg.clone(),
            public_key: aggregate_public_key,
            signature: aggregate_signature,
            aggregated: false,
            // TODO: FIXME.
            signer_id: 100,
        }
    }

    pub async fn run(&mut self) {
        loop {
            // Receive a message from another ground station
            // Receive a message from the space station
            tokio::select! {
                Ok(phase) = self.timer_rx.recv() => {
                    if phase == Phase::First {
                        self.slot_info.next();
                            info!(
                                "Slot {}\tGround Station {}",
                                self.slot_info.i,
                                self.station_id,
                            );
                    }
                    // For the rest we simply ignore.
                    // Handle Stop for breaking out of this run loop.
                }
                Some(request) = self.request_rx.recv() => {
                    if request.public_keys.contains(&self.public_key) {
                        return;
                    }
                    // FIXME: not sure how to make this mutable, without this line.
                    let mut request = request;
                    // Sign
                    request.signatures.push(self.sign(&request.msg));
                    request.public_keys.push(self.public_key.clone());

                    if request.signatures.len() >= supermajority(self.num_stations as usize) {
                        // Create precommit
                        let commit = self.aggregate(&request);
                        self.commit_tx.send(commit).await.unwrap();
                    }
                }
                Ok(commit) = self.commit_rx.recv() => {
                    if commit.typ() == CommitType::Precommit {
                        self.slot_info.j = commit.i;
                    }
                }
            }
        }
    }
}

// Ground Station
// Receives a message from clients, and generates precommit message.
//
// Needs to keep track of slot information, and it will be equivalent to the concept of 'round' in
// pbft like algorithms.
// Then, the gossip will happen similarly to the cubesats.
// In the initial version, it might be ok to send multiple precommits for a slot to the space station
// as the space station checks for it, but for optimization, only one of the ground stations
// should send a precommit for a slot.

// Each Listening Station will keep the information of the Last Committed slot, and
// it will be updated whenever the space station sends signed commit.

// There will be a loop that waiting for events to happen
// 1. Update slot information - timer
// 2. Receive a message from client
// 3. Receive a message from space station

// Define a message type for the client's request
// BounceRequest
//  - msg
//  - client's public key
//
// BounceResponse
//  - the original message
//  - slot information
//  - either commit or noncommit
//
