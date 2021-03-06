use crate::commit::CommitType;
use crate::{supermajority, Commit, Phase, SlotInfo};
use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use log::info;
use rand::{thread_rng, Rng};
use tokio::sync::{broadcast, mpsc};

pub enum FailureMode {
    // Follows the protocol and has no impostor.
    Honest = 1,
    // Sends precommit / noncommit messages at an arbitrary time.
    FailArbitrary,
    // Does not send precommit / noncommit messages at all.
    FailStop,
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

    failure_mode: FailureMode,
}

impl Cubesat {
    pub fn new(
        id: usize,
        num_cubesats: u32,
        result_tx: mpsc::Sender<Commit>,
        request_rx: mpsc::Receiver<Commit>,
        timer_rx: broadcast::Receiver<Phase>,
        failure_mode: FailureMode,
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
            failure_mode,
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
        info!(
            "Slot {}\tBounce Unit {}\tCommit Type {:?}\taggregated and broadcast",
            self.slot_info.i,
            self.id,
            commit.typ(),
        );
        self.result_tx.send(commit).await.unwrap();
    }

    async fn sign_and_broadcast(&mut self, mut commit: Commit) -> Commit {
        let signature = Bn256.sign(&self.private_key, &commit.msg).unwrap();
        commit.signature = signature;
        commit.public_key = self.public_key.to_vec();
        commit.i = self.slot_info.i;
        commit.signer_id = self.id as u32;

        self.slot_info.signed = true;
        self.result_tx.send(commit.clone()).await.unwrap();

        info!(
            "Slot {}\tBounce Unit {}\tCommit Type {:?}\tsign and broadcast",
            self.slot_info.i,
            self.id,
            commit.typ(),
        );

        commit
    }

    async fn process(&mut self, commit: Commit) {
        // Ignore the commit that was signed by itself.
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

        match self.failure_mode {
            FailureMode::Honest => self.process_honest(commit).await,
            FailureMode::FailArbitrary => self.process_fail_arbitrary(commit).await,
            FailureMode::FailStop => self.process_fail_stop(commit).await,
        }
    }

    async fn process_fail_arbitrary(&mut self, mut commit: Commit) {
        // Flip a coin to determine whether to send precommit or a noncommit.
        let typ = if thread_rng().gen::<f32>() < 0.5 {
            CommitType::Precommit
        } else {
            CommitType::Noncommit
        };

        // Overwrite the commit type.
        commit.set_typ(typ);

        if !self.slot_info.signed {
            commit = self.sign_and_broadcast(commit).await;
        }

        // Even though this is fail arbitrary, it will still follow the rest of the protocol, i.e.
        // keeping track of the number of precommits or noncommits.
        // TODO(taegyunk): Come up with a more reasonable scenario for this.
        if typ == CommitType::Precommit {
            self.slot_info.precommits.push(commit.clone());
        } else {
            self.slot_info.noncommits.push(commit.clone());
        }

        if self.slot_info.precommits.len() >= supermajority(self.num_cubesats as usize) {
            self.aggregate_and_broadcast(commit).await;
        } else if self.slot_info.noncommits.len() >= supermajority(self.num_cubesats as usize) {
            self.aggregate_and_broadcast(commit).await;
        }

        // TODO(taegyunk): Update to send the commit at a random time.
    }

    async fn process_fail_stop(&mut self, _commit: Commit) {
        // Does nothing
    }

    async fn process_honest(&mut self, mut commit: Commit) {
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
                            info!(
                                "Slot {}\tBounce Unit {}\tFirst Phase Starts",
                                self.slot_info.i,
                                self.id,
                            );
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
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bls_signatures_rs::MultiSignature;

    #[tokio::test]
    async fn cubesat_sign_aggregate() {
        let (result_tx, mut result_rx) = mpsc::channel(1);
        let (request_tx, request_rx) = mpsc::channel(15);
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 1, result_tx, request_rx, _timer_rx, FailureMode::Honest);
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
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 1, result_tx, request_rx, _timer_rx, FailureMode::Honest);

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
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 3, result_tx, request_rx, _timer_rx, FailureMode::Honest);

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
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 3, result_tx, request_rx, _timer_rx, FailureMode::Honest);

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
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 1, result_tx, request_rx, _timer_rx, FailureMode::Honest);

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
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 1, result_tx, request_rx, _timer_rx, FailureMode::Honest);

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
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 3, result_tx, request_rx, _timer_rx, FailureMode::Honest);
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
        let (_timer_tx, _timer_rx) = broadcast::channel(15);

        let mut c = Cubesat::new(0, 3, result_tx, request_rx, _timer_rx, FailureMode::Honest);

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
