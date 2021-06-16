use crate::commit::CommitType;
use crate::{supermajority, Commit};
use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use log::info;
use rand::{thread_rng, Rng};
use tokio::sync::{broadcast, mpsc};

#[derive(Clone, Debug, PartialEq)]
pub enum Phase {
    Stop,
    First,
    Second,
    Third,
}

#[derive(Clone, Debug)]
pub struct SlotInfo {
    // Index of current slot
    i: u32,
    // The index of last committed slot.
    j: u32,
    phase: Phase,
    // Whether this cubesat has signed a precommit or non-commit for current slot
    signed: bool,
    // Whether this cubesat has aggregated signatures of at least supermajority of num_cubesats
    aggregated: bool,
    // (id, signature) of precommtis or noncommits received for this slot.
    precommits: Vec<Commit>,
    noncommits: Vec<Commit>,
}

#[derive(Clone, Debug)]
pub enum Command {
    // Terminates the Cubesat and shuts off.
    Terminate,
    // Update slot info
}

impl SlotInfo {
    fn new() -> Self {
        Self {
            i: 0,
            j: 0,
            phase: Phase::Stop,
            signed: false,
            aggregated: false,
            precommits: Vec::new(),
            noncommits: Vec::new(),
        }
    }

    fn next(&mut self) {
        self.i += 1;
        self.phase = Phase::First;
        self.signed = false;
        self.aggregated = false;
        self.precommits.clear();
        self.noncommits.clear();

        info!("Starting slot {}", self.i);
    }
}

/// Bounce Unit invariants
/// 1. A Bounce unit will never send a precommit or non-commit if it has already sent a precommit
/// or non-commit
/// 2. A Bounce unit will never send a precommit or non-commit if it has already received an
/// aggregated precommit or non-commit or has sent one.
/// 3. A Bounce unit will never send an aggregated precommit or non-commit if it has either received
/// an aggregated precommit or non-commit or has already sent one.
pub struct Cubesat {
    id: usize,

    // Configuration for slot
    num_cubesats: u32,
    slot_info: SlotInfo,

    public_key: Vec<u8>,
    private_key: Vec<u8>,

    // sender to send to communications hub
    result_tx: mpsc::Sender<Commit>,
    // receiver to receive Commits from the communications hub
    request_rx: mpsc::Receiver<Commit>,

    // Receiver for phase transitions.
    timer_rx: broadcast::Receiver<Phase>,

    // receiver for commands
    command_rx: mpsc::Receiver<Command>,
}

impl Cubesat {
    pub fn new(
        id: usize,
        num_cubesats: u32,
        result_tx: mpsc::Sender<Commit>,
        request_rx: mpsc::Receiver<Commit>,
        timer_rx: broadcast::Receiver<Phase>,
        command_rx: mpsc::Receiver<Command>,
    ) -> Self {
        let mut rng = thread_rng();

        // generate public and private key pairs.
        let private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let public_key = Bn256.derive_public_key(&private_key).unwrap();
        let slot_info = SlotInfo::new();

        Cubesat {
            id,
            num_cubesats,
            slot_info,
            public_key,
            private_key,
            result_tx,
            request_rx,
            timer_rx,
            command_rx,
        }
    }

    fn aggregate(commits: &[Commit]) -> (Vec<u8>, Vec<u8>) {
        let sig_refs: Vec<&[u8]> = commits.iter().map(|c| c.signature.as_slice()).collect();
        let aggregate_signature = Bn256.aggregate_signatures(&sig_refs).unwrap();

        let public_key_refs: Vec<&[u8]> = commits.iter().map(|c| c.public_key.as_slice()).collect();
        let aggregate_public_key = Bn256.aggregate_public_keys(&public_key_refs).unwrap();

        (aggregate_signature, aggregate_public_key)
    }

    fn get_commits(&self, commit_type: CommitType) -> &[Commit] {
        if commit_type == CommitType::Precommit {
            &self.slot_info.precommits
        } else {
            &self.slot_info.noncommits
        }
    }

    async fn aggregate_and_broadcast(&mut self, mut commit: Commit) {
        info!("Bounce unit {}: aggregating {:?}", self.id, commit.typ());
        let (aggregate_signature, aggregate_public_key) =
            Cubesat::aggregate(self.get_commits(commit.typ()));

        commit.signature = aggregate_signature;
        commit.public_key = aggregate_public_key;
        commit.aggregated = true;
        commit.i = self.slot_info.i;
        commit.signer_id = self.id as u32;

        self.slot_info.aggregated = true;
        if commit.typ() == CommitType::Precommit {
            self.slot_info.j = commit.i;
        }
        info!("Unit {} aggregated and broadcast", self.id);
        self.result_tx.send(commit).await.unwrap();
    }

    async fn sign_and_broadcast(&mut self, mut commit: Commit) -> Commit {
        info!("Bounce unit {}: signed a {:?}", self.id, commit.typ());
        let signature = Bn256.sign(&self.private_key, &commit.msg).unwrap();
        commit.signature = signature;
        commit.public_key = self.public_key.to_vec();
        commit.i = self.slot_info.i;
        commit.signer_id = self.id as u32;

        self.slot_info.signed = true;
        self.result_tx.send(commit.clone()).await.unwrap();

        commit
    }

    async fn process(&mut self, mut commit: Commit) {
        if self.public_key == commit.public_key {
            return;
        }

        if self.slot_info.phase == Phase::Stop {
            return;
        }

        // If thie Bounce unit has already aggregated or received an aggregate signature, then just
        // return.
        if self.slot_info.aggregated {
            return;
        }

        // If the commit is an aggregate signature, then we note that this slot is aggregated and
        // update the last committed slot and current slot information.
        if commit.aggregated && commit.i == self.slot_info.i {
            self.slot_info.aggregated = true;
            self.slot_info.j = commit.j;
            return;
        }

        match self.slot_info.phase {
            Phase::First => {
                // Phase 1 only handles precommits
                if commit.typ() == CommitType::Precommit {
                    if !self.slot_info.signed {
                        commit = self.sign_and_broadcast(commit).await;
                    }

                    // Now, the precommit is the one signed by me or other cubesats.
                    self.slot_info.precommits.push(commit.clone());
                }
            }
            Phase::Second => {
                // Sign
                if !self.slot_info.signed {
                    commit = self.sign_and_broadcast(commit).await;
                }

                if commit.typ() == CommitType::Precommit {
                    self.slot_info.precommits.push(commit.clone());
                } else if commit.typ() == CommitType::Noncommit {
                    self.slot_info.noncommits.push(commit.clone());
                }
            }
            Phase::Third => {
                // At the beginning of the Phase 3, this Bounce unit has signed and broadcast
                // a noncommit, so it will only listen to others' commits.
                if commit.typ() == CommitType::Precommit {
                    self.slot_info.precommits.push(commit.clone());
                } else if commit.typ() == CommitType::Noncommit {
                    self.slot_info.noncommits.push(commit.clone());
                }
            }
            Phase::Stop => {
                unreachable!("Handled Stop phase earlier in the function.");
            }
        }

        if self.slot_info.precommits.len() >= supermajority(self.num_cubesats as usize) {
            self.aggregate_and_broadcast(commit).await;
        } else if self.slot_info.noncommits.len() >= supermajority(self.num_cubesats as usize) {
            self.aggregate_and_broadcast(commit).await;
        }
    }

    pub async fn run(&mut self) {
        loop {
            tokio::select! {
                Ok(phase) = self.timer_rx.recv() => {
                    match phase {
                        Phase::First => {
                            self.slot_info.next();
                        }
                        Phase::Second => {
                        }
                        Phase::Third => {
                            if !self.slot_info.signed {
                                // Sign and broadcast noncommit for (j+1, i)

                                let msg = format!("noncommit({}, {})", self.slot_info.j + 1, self.slot_info.i);

                                let noncommit = Commit {
                                    typ: CommitType::Noncommit.into(),
                                    i: self.slot_info.i,
                                    j: self.slot_info.j,
                                    msg: msg.clone().into_bytes(),
                                    public_key: self.public_key.clone(),
                                    signature: Bn256.sign(&self.private_key, &msg.as_bytes()).unwrap(),
                                    aggregated: false,
                                    signer_id: self.id as u32,
                                };
                                self.sign_and_broadcast(
                                    noncommit.clone()
                                ).await;
                                self.slot_info.noncommits.push(noncommit);
                            }
                        }
                        Phase::Stop => {

                        }
                    }
                    self.slot_info.phase = phase;
                }
                Some(commit) = self.request_rx.recv() => {
                    self.process(commit).await;
                }
                Some(cmd) = self.command_rx.recv() => {
                    match cmd {
                        Command::Terminate => {
                            info!("exiting...");
                            break;
                        }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use bls_signatures_rs::MultiSignature;

    use super::*;

    #[tokio::test]
    async fn cubesat_terminate() {
        let (result_tx, _) = mpsc::channel(1);
        let (_request_tx, request_rx) = mpsc::channel(1);
        let (command_tx, command_rx) = mpsc::channel(10);
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 1, result_tx, request_rx, _timer_rx, command_rx);

        tokio::spawn(async move {
            c.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        command_tx
            .send(Command::Terminate)
            .await
            .expect("Failed to send terminate command");
    }

    #[tokio::test]
    async fn cubesat_sign_aggregate() {
        let (result_tx, mut result_rx) = mpsc::channel(1);
        let (request_tx, request_rx) = mpsc::channel(15);
        let (_command_tx, command_rx) = mpsc::channel(10);
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 1, result_tx, request_rx, _timer_rx, command_rx);
        c.slot_info.phase = Phase::First;

        tokio::spawn(async move {
            c.run().await;
        });

        let msg = "hello".as_bytes().to_vec();

        let mut rng = thread_rng();
        let ground_station_private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let ground_station_public_key = Bn256
            .derive_public_key(&ground_station_private_key)
            .unwrap();
        let signature = Bn256.sign(&ground_station_private_key, &msg).unwrap();

        let precommit = Commit {
            typ: CommitType::Precommit.into(),
            i: 0,
            j: 0,
            msg: msg.clone(),
            public_key: ground_station_public_key,
            signature,
            aggregated: false,
            signer_id: 0,
        };

        tokio::spawn(async move {
            request_tx
                .send(precommit)
                .await
                .expect("failed to send precommit");
        });

        let result_opt = result_rx.recv().await;
        assert!(result_opt.is_some());
        let commit = result_opt.unwrap();

        assert_eq!(commit.typ(), CommitType::Precommit);
        assert_eq!(commit.i, 0);
        assert_eq!(commit.msg, msg);
        assert!(!commit.aggregated);

        let result_opt = result_rx.recv().await;
        assert!(result_opt.is_some());
        let commit = result_opt.unwrap();

        assert_eq!(commit.typ(), CommitType::Precommit);
        assert_eq!(commit.i, 0);
        assert_eq!(commit.msg, msg);
        assert!(commit.aggregated);

        let _ = Bn256
            .verify(&commit.signature, &msg, &commit.public_key)
            .unwrap();
    }

    #[tokio::test]
    async fn phase1_noncommit() {
        let (result_tx, _result_rx) = mpsc::channel(1);
        let (_request_tx, request_rx) = mpsc::channel(15);
        let (_command_tx, command_rx) = mpsc::channel(10);
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 1, result_tx, request_rx, _timer_rx, command_rx);

        c.slot_info.phase = Phase::First;

        assert!(!c.slot_info.signed);
        assert!(!c.slot_info.aggregated);

        let noncommit = Commit {
            typ: CommitType::Noncommit.into(),
            i: 1,
            j: 0,
            aggregated: false,
            public_key: Vec::new(),
            msg: Vec::new(),
            signature: Vec::new(),
            signer_id: 0,
        };

        c.process(noncommit).await;

        assert!(!c.slot_info.signed);
        assert!(!c.slot_info.aggregated);
    }

    #[tokio::test]
    async fn phase2_commit_noncommit() {
        // Phase 2, first send commit, then noncommit. Then the Bounce unit should sign the commit
        // and broadcast. If it receives the noncommit right after, then bounce unit should not sign
        // the noncommit and it only needs to keep track that it has received a noncommit.

        let (result_tx, mut result_rx) = mpsc::channel(1);
        let (_request_tx, request_rx) = mpsc::channel(15);
        let (_command_tx, command_rx) = mpsc::channel(10);
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 3, result_tx, request_rx, _timer_rx, command_rx);

        c.slot_info.phase = Phase::Second;

        assert!(!c.slot_info.signed);
        assert!(!c.slot_info.aggregated);
        assert!(c.slot_info.precommits.is_empty());

        let msg = "hello".as_bytes().to_vec();

        let mut rng = thread_rng();
        let cubesat1_private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let cubesat1_public_key = Bn256.derive_public_key(&cubesat1_private_key).unwrap();
        let signature = Bn256.sign(&cubesat1_private_key, &msg).unwrap();

        let precommit = Commit {
            typ: CommitType::Precommit.into(),
            i: 0,
            j: 0,
            msg: msg.clone(),
            public_key: cubesat1_public_key,
            signature,
            aggregated: false,
            signer_id: 0,
        };

        c.process(precommit).await;
        assert!(c.slot_info.signed);
        assert!(!c.slot_info.aggregated);
        assert_eq!(c.slot_info.precommits.len(), 1);

        let result_opt = result_rx.recv().await;
        assert!(result_opt.is_some());
        let commit = result_opt.unwrap();
        assert_eq!(commit.typ(), CommitType::Precommit);
        assert_eq!(commit.i, 0);
        assert_eq!(commit.msg, msg);
        assert_eq!(commit.public_key, c.public_key);

        let cubesat2_private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let cubesat2_public_key = Bn256.derive_public_key(&cubesat2_private_key).unwrap();
        let signature = Bn256.sign(&cubesat2_private_key, &msg).unwrap();

        let noncommit = Commit {
            typ: CommitType::Noncommit.into(),
            i: 1,
            j: 0,
            msg: msg.clone(),
            public_key: cubesat2_public_key,
            signature,
            aggregated: false,
            signer_id: 0,
        };

        c.process(noncommit).await;
        assert_eq!(1, c.slot_info.noncommits.len());
    }

    #[tokio::test]
    async fn phase2_noncommit_commit() {
        // Similar as above, it only signs the first noncommit, and not the commit. Only keep track
        // of the commit.

        let (result_tx, mut result_rx) = mpsc::channel(1);
        let (_request_tx, request_rx) = mpsc::channel(15);
        let (_command_tx, command_rx) = mpsc::channel(10);
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 3, result_tx, request_rx, _timer_rx, command_rx);

        c.slot_info.phase = Phase::Second;

        assert!(!c.slot_info.signed);
        assert!(!c.slot_info.aggregated);
        assert!(c.slot_info.precommits.is_empty());

        let msg = "hello".as_bytes().to_vec();

        let mut rng = thread_rng();
        let cubesat1_private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let cubesat1_public_key = Bn256.derive_public_key(&cubesat1_private_key).unwrap();
        let signature = Bn256.sign(&cubesat1_private_key, &msg).unwrap();

        let noncommit = Commit {
            typ: CommitType::Noncommit.into(),
            i: 1,
            j: 0,
            msg: msg.clone(),
            public_key: cubesat1_public_key,
            signature,
            aggregated: false,
            signer_id: 0,
        };

        c.process(noncommit).await;
        assert!(c.slot_info.signed);
        assert!(!c.slot_info.aggregated);
        assert_eq!(c.slot_info.noncommits.len(), 1);

        let result_opt = result_rx.recv().await;
        assert!(result_opt.is_some());
        let commit = result_opt.unwrap();
        assert_eq!(commit.typ(), CommitType::Noncommit);
        assert_eq!(commit.i, 0);
        assert_eq!(commit.msg, msg);
        assert_eq!(commit.public_key, c.public_key);

        let cubesat2_private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let cubesat2_public_key = Bn256.derive_public_key(&cubesat2_private_key).unwrap();
        let signature = Bn256.sign(&cubesat2_private_key, &msg).unwrap();

        let precommit = Commit {
            typ: CommitType::Precommit.into(),
            i: 1,
            j: 0,
            msg: msg.clone(),
            public_key: cubesat2_public_key,
            signature,
            aggregated: false,
            signer_id: 0,
        };

        c.process(precommit).await;
        assert_eq!(1, c.slot_info.noncommits.len());
    }

    #[tokio::test]
    async fn phase2_commit_aggregate() {
        // Tests that in phase 2 the bounce unit aggregates signatures.
        let (result_tx, _result_rx) = mpsc::channel(5);
        let (_request_tx, request_rx) = mpsc::channel(15);
        let (_command_tx, command_rx) = mpsc::channel(10);
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 1, result_tx, request_rx, _timer_rx, command_rx);

        c.slot_info.phase = Phase::Second;

        assert!(!c.slot_info.signed);
        assert!(!c.slot_info.aggregated);
        assert!(c.slot_info.precommits.is_empty());

        let msg = "hello".as_bytes().to_vec();

        let mut rng = thread_rng();
        let cubesat1_private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let cubesat1_public_key = Bn256.derive_public_key(&cubesat1_private_key).unwrap();
        let signature = Bn256.sign(&cubesat1_private_key, &msg).unwrap();

        let precommit = Commit {
            typ: CommitType::Precommit.into(),
            i: 1,
            j: 0,
            msg: msg.clone(),
            public_key: cubesat1_public_key,
            signature,
            aggregated: false,
            signer_id: 0,
        };

        c.process(precommit).await;
        assert!(c.slot_info.signed);
        assert!(c.slot_info.aggregated);
        assert_eq!(c.slot_info.precommits.len(), 1);
    }

    #[tokio::test]
    async fn phase2_noncommit_aggregate() {
        // Tests that in phase 2 the bounce unit aggregates signatures.
        let (result_tx, _result_rx) = mpsc::channel(5);
        let (_request_tx, request_rx) = mpsc::channel(15);
        let (_command_tx, command_rx) = mpsc::channel(10);
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 1, result_tx, request_rx, _timer_rx, command_rx);

        c.slot_info.phase = Phase::Second;

        assert!(!c.slot_info.signed);
        assert!(!c.slot_info.aggregated);
        assert!(c.slot_info.noncommits.is_empty());

        let msg = "hello".as_bytes().to_vec();

        let mut rng = thread_rng();
        let cubesat1_private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let cubesat1_public_key = Bn256.derive_public_key(&cubesat1_private_key).unwrap();
        let signature = Bn256.sign(&cubesat1_private_key, &msg).unwrap();

        let noncommit = Commit {
            typ: CommitType::Noncommit.into(),
            i: 1,
            j: 0,
            msg: msg.clone(),
            public_key: cubesat1_public_key,
            signature,
            aggregated: false,
            signer_id: 0,
        };

        c.process(noncommit).await;
        assert!(c.slot_info.signed);
        assert!(c.slot_info.aggregated);
        assert_eq!(c.slot_info.noncommits.len(), 1);
    }

    #[tokio::test]
    async fn phase3_receives_precommit() {
        let (result_tx, _result_rx) = mpsc::channel(5);
        let (_request_tx, request_rx) = mpsc::channel(15);
        let (_command_tx, command_rx) = mpsc::channel(10);
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 3, result_tx, request_rx, _timer_rx, command_rx);
        // Assume that this Bounce unit has entered into the third phase, and signed a noncommit.
        c.slot_info.phase = Phase::Third;
        let msg = format!("noncommit({}, {})", c.slot_info.j + 1, c.slot_info.i);
        let noncommit = Commit {
            typ: CommitType::Noncommit.into(),
            i: c.slot_info.i,
            j: c.slot_info.j,
            msg: msg.clone().into_bytes(),
            public_key: c.public_key.clone(),
            signature: Bn256.sign(&c.private_key, &msg.as_bytes()).unwrap(),
            aggregated: false,
            signer_id: 0,
        };

        c.sign_and_broadcast(noncommit.clone()).await;
        c.slot_info.noncommits.push(noncommit);

        let msg = "hello".as_bytes().to_vec();

        // Then another Bounce unit sends it precommit, and the Bounce unit just keeps track of it.
        let mut rng = thread_rng();
        let cubesat1_private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let cubesat1_public_key = Bn256.derive_public_key(&cubesat1_private_key).unwrap();
        let signature = Bn256.sign(&cubesat1_private_key, &msg).unwrap();

        let precommit = Commit {
            typ: CommitType::Precommit.into(),
            i: c.slot_info.i,
            j: c.slot_info.j,
            msg,
            public_key: cubesat1_public_key,
            signature,
            aggregated: false,
            signer_id: 0,
        };

        c.process(precommit).await;

        assert!(!c.slot_info.aggregated);
        assert_eq!(c.slot_info.precommits.len(), 1);
        assert_eq!(c.slot_info.noncommits.len(), 1);
    }

    #[tokio::test]
    async fn phase3_sign_noncommit_aggregate() {
        let (result_tx, _result_rx) = mpsc::channel(5);
        let (_request_tx, request_rx) = mpsc::channel(15);
        let (_command_tx, command_rx) = mpsc::channel(10);
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 3, result_tx, request_rx, _timer_rx, command_rx);

        // Assume that this Bounce unit has entered into the third phase, and signed a noncommit.
        c.slot_info.phase = Phase::Third;
        let msg = format!("noncommit({}, {})", c.slot_info.j + 1, c.slot_info.i);
        let noncommit = Commit {
            typ: CommitType::Noncommit.into(),
            i: c.slot_info.i,
            j: c.slot_info.j,
            msg: msg.clone().into_bytes(),
            public_key: c.public_key.clone(),
            signature: Bn256.sign(&c.private_key, &msg.as_bytes()).unwrap(),
            aggregated: false,
            signer_id: 0,
        };

        c.sign_and_broadcast(noncommit.clone()).await;
        c.slot_info.noncommits.push(noncommit);

        assert!(c.slot_info.signed);
        assert!(!c.slot_info.aggregated);
        assert_eq!(c.slot_info.noncommits.len(), 1);

        // Then another Bounce unit sends it noncommit, which results in aggregation.
        let mut rng = thread_rng();
        let cubesat1_private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let cubesat1_public_key = Bn256.derive_public_key(&cubesat1_private_key).unwrap();
        let signature = Bn256.sign(&cubesat1_private_key, &msg.as_bytes()).unwrap();

        let noncommit = Commit {
            typ: CommitType::Noncommit.into(),
            i: c.slot_info.i,
            j: c.slot_info.j,
            msg: msg.into_bytes(),
            public_key: cubesat1_public_key,
            signature,
            aggregated: false,
            signer_id: 0,
        };

        c.process(noncommit).await;
        assert!(c.slot_info.signed);
        assert!(c.slot_info.aggregated);
        assert_eq!(c.slot_info.noncommits.len(), 2);
    }
}
