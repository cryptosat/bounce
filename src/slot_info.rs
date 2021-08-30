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
    // Whether this cubesat has aggregated signatures of at least supermajority of num_bounce_units
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::commit::CommitType;

    #[test]
    fn slot_info_init_test() {
        let slot_info = SlotInfo::new();

        assert_eq!(slot_info.i, 0);
        assert_eq!(slot_info.j, 0);
        assert_eq!(slot_info.phase, Phase::Stop);
        assert!(!slot_info.signed);
        assert!(!slot_info.aggregated);
        assert!(slot_info.precommits.is_empty());
        assert!(slot_info.noncommits.is_empty());
    }

    #[test]
    fn slot_info_next_test() {
        let mut slot_info = SlotInfo::new();
        slot_info.phase = Phase::Second;
        slot_info.signed = true;
        slot_info.aggregated = true;
        slot_info.noncommits.push(Commit {
            typ: CommitType::Noncommit.into(),
            i: 0,
            j: 0,
            msg: "test".to_owned().into_bytes(),
            public_key: "dummy key".to_owned().into_bytes(),
            signature: "dummy signature".to_owned().into_bytes(),
            aggregated: false,
            signer_id: 0,
        });

        slot_info.next();
        assert_eq!(slot_info.i, 1);
        assert_eq!(slot_info.phase, Phase::First);
        assert!(!slot_info.signed);
        assert!(!slot_info.aggregated);
        assert!(slot_info.noncommits.is_empty());
    }
}
