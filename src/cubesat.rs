use crate::commit::CommitType;
use crate::{supermajority, BounceConfig, Commit};
use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use rand::{thread_rng, Rng};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{interval, interval_at, Instant};

#[derive(Clone, Debug)]
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
        if self.signed {
            self.j = self.i;
        }
        self.i += 1;
        self.phase = Phase::First;
        self.signed = false;
        self.aggregated = false;
        self.precommits.clear();
        self.noncommits.clear();
    }
}

pub struct Cubesat {
    id: usize,

    // Configuration for slot
    bounce_config: BounceConfig,
    slot_info: SlotInfo,

    public_key: Vec<u8>,
    private_key: Vec<u8>,

    // sender to send to communications hub
    result_tx: mpsc::Sender<Commit>,
    // receiver to receive Commits from the communications hub
    request_rx: mpsc::Receiver<Commit>,

    // receiver for commands
    command_rx: mpsc::Receiver<Command>,
}

impl Cubesat {
    pub fn new(
        id: usize,
        bounce_config: BounceConfig,
        result_tx: mpsc::Sender<Commit>,
        request_rx: mpsc::Receiver<Commit>,
        command_rx: mpsc::Receiver<Command>,
    ) -> Self {
        let mut rng = thread_rng();

        // generate public and private key pairs.
        let private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let public_key = Bn256.derive_public_key(&private_key).unwrap();
        let slot_info = SlotInfo::new();

        Cubesat {
            id,
            bounce_config,
            slot_info,
            public_key,
            private_key,
            result_tx,
            request_rx,
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

    async fn process(&mut self, commit: Commit) {
        match self.slot_info.phase {
            Phase::First => {
                if commit.typ() == CommitType::Noncommit {
                    // Ignore noncommit
                    return;
                }

                let mut precommit = commit.clone();

                // If already aggregated, just update the slot information
                if precommit.aggregated || self.slot_info.aggregated {
                    self.slot_info.aggregated = true;
                    self.slot_info.i = precommit.i;
                    self.slot_info.j = precommit.i;
                    return;
                }

                // If this didn't sign, then sign and broadcast.
                if !self.slot_info.signed {
                    // Sign
                    let signature = Bn256.sign(&self.private_key, &commit.msg).unwrap();
                    println!("Signed");
                    let mut precommit = commit.clone();
                    precommit.signature = signature;
                    precommit.public_key = self.public_key.to_vec();
                    self.slot_info.precommits.push(precommit.clone());
                    self.slot_info.signed = true;
                    self.result_tx.send(precommit).await.unwrap();
                }
                self.slot_info.precommits.push(precommit.clone());

                // Aggregate
                if self.slot_info.precommits.len()
                    >= supermajority(self.bounce_config.num_cubesats as usize)
                {
                    println!("{} aggregated", self.id);
                    let (aggregate_signature, aggregate_public_key) =
                        Cubesat::aggregate(&self.slot_info.precommits);

                    precommit.signature = aggregate_signature;
                    precommit.public_key = aggregate_public_key;
                    precommit.aggregated = true;

                    self.slot_info.aggregated = true;
                    self.slot_info.j = precommit.i;
                    self.result_tx.send(precommit).await.unwrap();
                }
            }
            Phase::Second => {
                if commit.aggregated || self.slot_info.aggregated {
                    self.slot_info.aggregated = true;

                    self.slot_info.i = commit.i;
                    if commit.typ() == CommitType::Precommit {
                        self.slot_info.j = commit.i;
                    } else if commit.typ() == CommitType::Noncommit {
                        self.slot_info.j = commit.j;
                    }

                    return;
                }

                // Sign
                if !self.slot_info.signed {
                    let signature = Bn256.sign(&self.private_key, &commit.msg).unwrap();
                    let mut commit = commit.clone();
                    commit.signature = signature;
                    commit.public_key = self.public_key.to_vec();

                    if commit.typ() == CommitType::Precommit {
                        self.slot_info.precommits.push(commit.clone());
                    } else if commit.typ() == CommitType::Noncommit {
                        self.slot_info.noncommits.push(commit.clone());
                    }
                    self.slot_info.signed = true;
                    self.result_tx.send(commit).await.unwrap();
                }

                // Aggregate if > supermajority
                // Broadcast
            }
            Phase::Third => {
                if commit.typ() == CommitType::Precommit {
                    // ignore precommit
                    return;
                }
                // Sign
                // Aggregate noncommits if > supermajority
                // Broadcast
            }
            Phase::Stop => {
                // Does nothing.
                return;
            }
        }
    }

    pub async fn run(&mut self) {
        let slot_duration = Duration::from_secs(self.bounce_config.slot_duration as u64);
        let mut slot_ticker = interval(slot_duration);
        let start = Instant::now();
        let phase2_start = start + Duration::from_secs(self.bounce_config.phase1_duration as u64);
        let phase3_start =
            phase2_start + Duration::from_secs(self.bounce_config.phase2_duration as u64);
        let mut phase2_ticker = interval_at(phase2_start, slot_duration);
        let mut phase3_ticker = interval_at(phase3_start, slot_duration);

        self.slot_info.phase = Phase::First;
        loop {
            tokio::select! {
                // _ = slot_ticker.tick() => {

                //     // self.slot_info.next();
                //     println!("slot timer tick");
                // }
                // _ = phase2_ticker.tick() => {
                //     self.slot_info.phase = Phase::Second;
                // }
                // _ = phase3_ticker.tick() => {
                //     // Have to sign and send noncommit for (j + 1, i)

                //     self.slot_info.phase = Phase::Third;

                // }
                Some(commit) = self.request_rx.recv() => {
                    self.process(commit).await;
                }
                Some(cmd) = self.command_rx.recv() => {
                    match cmd {
                        Command::Terminate => {
                            println!("exiting...");
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
    use super::*;

    #[tokio::test]
    async fn cubesat_terminate() {
        let (result_tx, _) = mpsc::channel(1);
        let (_request_tx, request_rx) = mpsc::channel(1);
        let (command_tx, command_rx) = mpsc::channel(10);

        let mut c = Cubesat::new(
            0,
            BounceConfig {
                num_cubesats: 1,
                slot_duration: 10,
                phase1_duration: 4,
                phase2_duration: 4,
            },
            result_tx,
            request_rx,
            command_rx,
        );

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

        let mut c = Cubesat::new(
            0,
            BounceConfig {
                num_cubesats: 1,
                slot_duration: 10,
                phase1_duration: 4,
                phase2_duration: 4,
            },
            result_tx,
            request_rx,
            command_rx,
        );

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
            i: 1,
            j: 0,
            msg: msg.clone(),
            public_key: ground_station_public_key,
            signature,
            aggregated: false,
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
        assert_eq!(commit.i, 1);
        assert_eq!(commit.msg, msg);
        assert!(!commit.aggregated);

        let result_opt = result_rx.recv().await;
        assert!(result_opt.is_some());
        let commit = result_opt.unwrap();

        assert_eq!(commit.typ(), CommitType::Precommit);
        assert_eq!(commit.i, 1);
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

        let mut c = Cubesat::new(
            0,
            BounceConfig {
                num_cubesats: 1,
                slot_duration: 10,
                phase1_duration: 4,
                phase2_duration: 4,
            },
            result_tx,
            request_rx,
            command_rx,
        );

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
        };

        c.process(noncommit).await;

        assert!(!c.slot_info.signed);
        assert!(!c.slot_info.aggregated);
    }
}
