use crate::Commit;

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Stop,
    First,
    Second,
    Third,
}

impl Default for Phase {
    fn default() -> Phase {
        Phase::Stop
    }
}

#[derive(Clone, Debug, Default)]
pub struct SlotInfo {
    // Index of current slot
    pub i: u32,
    // The index of last committed slot.
    pub j: u32,
    pub phase: Phase,
    // Whether this cubesat has signed a precommit or non-commit for current slot
    pub signed: bool,
    // Whether this cubesat has aggregated signatures of at least supermajority of num_cubesats
    pub aggregated: bool,
    // (id, signature) of precommtis or noncommits received for this slot.
    pub precommits: Vec<Commit>,
    pub noncommits: Vec<Commit>,
}

impl SlotInfo {
    pub fn new() -> Self {
        Default::default()
    }

    pub fn next(&mut self) {
        self.i += 1;
        self.phase = Phase::First;
        self.signed = false;
        self.aggregated = false;
        self.precommits.clear();
        self.noncommits.clear();
    }
}
