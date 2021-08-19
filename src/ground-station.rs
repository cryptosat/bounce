pub enum StationType {
    Sending,
    Listening,
}

pub struct GroundStation {
    pub station_id: u32,
    pub station_type: StationType,
}

impl GroundStation {
    pub fn new(station_id: u32, station_type: StationType) -> GroundStation {
        GroundStation {
            station_id,
            station_type,
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
