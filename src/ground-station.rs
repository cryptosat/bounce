use crate::{supermajority, Commit, Phase, SlotInfo};
use tokio::sync::{broadcast, mpsc};

pub enum StationType {
    Sending = 0,
    Listening,
}

pub struct GroundStation {
    pub station_id: u32,
    pub station_type: StationType,

    public_key: Vec<u8>,
    private_key: Vec<u8>,

    slot_info: SlotInfo,
    // Receiver for phase transitions.
    timer_rx: broadcast::Receiver<Phase>,
}

impl GroundStation {
    pub fn new(
        station_id: u32,
        station_type: StationType,
        timer_rx: broadcast::Receiver<Phase>,

    ) -> GroundStation {
        let mut rng = thread_rng();

        // generate public and private key pairs.
        let private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let public_key = Bn256.derive_public_key(&private_key).unwrap();
        let slot_info = SlotInfo::new();

        GroundStation {
            station_id,
            station_type,
            public_key,
            private_key,
            timer_rx,
        }
    }

    fn sign(&self) {}

    fn aggregate(&self) {}

    async fn broadcast(&self) {}

    pub async fn run(&mut self) {
        loop {
            // Receive a message from another ground station
            // Receive a message from the space station
            tokio::select! {
                Ok(phase) = self.timer_rx.recv() => {
                    match phase {
                        Phase::First => {
                            self.slot_info.next();
                            info!(
                                "Slot {}\tGround Station {}",
                                self.slot_info.i,
                                self.station_id,
                            );
                        }
                        _ => {
                            // For the rest we simply ignore.
                            // Handle Stop for breaking out of this run loop.
                        }
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