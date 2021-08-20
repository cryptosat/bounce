pub enum StationType {
    Sending,
    Listening,
}

pub struct GroundStation {
    pub station_id: u32,
    pub station_type: StationType,

    public_key: Vec<u8>,
    private_key: Vec<u8>,
}

impl GroundStation {
    pub fn new(station_id: u32, station_type: StationType) -> GroundStation {
        let mut rng = thread_rng();

        // generate public and private key pairs.
        let private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let public_key = Bn256.derive_public_key(&private_key).unwrap();

        GroundStation {
            station_id,
            station_type,
            public_key,
            private_key,
        }
    }

    fn sign(&self) {}

    fn aggregate(&self) {}

    async fn broadcast(&self) {}

    pub async fn run(&mut self) {
        loop {
            // Receive a message from another ground station
            // Receive a message from the space station
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
