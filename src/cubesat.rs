use super::{BounceConfig, Commit, CommitType};
use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use rand::{thread_rng, Rng};
use std::time::Duration;
use tokio::sync::mpsc;
use tokio::time::{interval, interval_at, Instant};

#[derive(Clone, Debug)]
pub enum Command {
    Stop,
    // Sign this message sent from ground station
    Sign(Vec<u8>),
    // Aggregate either precommit or noncommit
    Aggregate(Commit),
}

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
    idx: usize,
    // The index of last committed slot.
    j: usize,
    phase: Phase,
    // Whether this cubesat has signed a precommit or non-commit for current slot
    signed: bool,
    // Whether this cubesat has aggregated signatures of at least supermajority of num_cubesats
    aggregated: bool,
    // (id, signature) of precommtis or noncommits received for this slot.
    precommits: Vec<Commit>,
    noncommits: Vec<Commit>,
}

impl SlotInfo {
    fn new() -> Self {
        Self {
            idx: 0,
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
            self.j = self.idx;
        }
        self.idx += 1;
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
    result_tx: mpsc::Sender<Command>,
    // receiver to receive commands from the communications hub
    request_rx: mpsc::Receiver<Command>,
}

impl Cubesat {
    pub fn new(
        id: usize,
        bounce_config: BounceConfig,
        result_tx: mpsc::Sender<Command>,
        request_rx: mpsc::Receiver<Command>,
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
        }
    }

    async fn process(&mut self, commit: Commit) {
        match self.slot_info.phase {
            Phase::First => {
                if commit.typ == CommitType::Noncommit {
                    // Ignore noncommit
                    return;
                }
                // Sign
                // Aggregate if > supermajority
                // Broadcast
            }
            Phase::Second => {
                // Sign
                // Aggregate if > supermajority
                // Broadcast
            }
            Phase::Third => {
                if commit.typ == CommitType::Precommit {
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

    async fn sign(&mut self, msg: &[u8]) {
        if self.slot_info.signed {
            // already signed for this slot.
            return;
        }

        let signature = Bn256.sign(&self.private_key, &msg).unwrap();

        // TODO: check errors
        self.result_tx
            .send(Command::Aggregate(Commit {
                typ: CommitType::Precommit,
                id: self.id,
                msg: msg.to_vec(),
                public_key: self.public_key.clone(),
                signature,
            }))
            .await
            .unwrap();
    }

    async fn aggregate(&mut self, commit: Commit) {
        match commit.typ {
            CommitType::Precommit => {
                self.slot_info.precommits.push(commit);

                if self.slot_info.precommits.len() == self.bounce_config.num_cubesats {
                    let sig_refs: Vec<&[u8]> = self
                        .slot_info
                        .precommits
                        .iter()
                        .map(|p| p.signature.as_slice())
                        .collect();
                    let _aggregate_signature = Bn256.aggregate_signatures(&sig_refs).unwrap();
                    let public_key_refs: Vec<&[u8]> = self
                        .slot_info
                        .precommits
                        .iter()
                        .map(|p| p.public_key.as_slice())
                        .collect();
                    let _aggregate_public_key =
                        Bn256.aggregate_public_keys(&public_key_refs).unwrap();
                }
            }
            CommitType::Noncommit => {
                self.slot_info.noncommits.push(commit);
            }
        }
    }

    pub async fn run(&mut self) {
        let slot_duration = Duration::from_secs(self.bounce_config.slot_duration);
        let mut slot_ticker = interval(slot_duration);
        let start = Instant::now();
        let phase2_start = start + Duration::from_secs(self.bounce_config.phase1_duration);
        let phase3_start = phase2_start + Duration::from_secs(self.bounce_config.phase2_duration);
        let mut phase2_ticker = interval_at(phase2_start, slot_duration);
        let mut phase3_ticker = interval_at(phase3_start, slot_duration);

        loop {
            tokio::select! {
                _ = slot_ticker.tick() => {

                    self.slot_info.next();
                    println!("slot timer tick");
                }
                _ = phase2_ticker.tick() => {
                    self.slot_info.phase = Phase::Second;
                }
                _ = phase3_ticker.tick() => {
                    // Have to sign and send noncommit for (j + 1, i)

                    self.slot_info.phase = Phase::Third;

                }
                Some(cmd) = self.request_rx.recv() => {
                    match cmd {
                        Command::Stop => {
                            self.slot_info.phase = Phase::Stop;
                            println!("exiting the loop...");
                            break;
                        }
                        Command::Sign(msg) => {
                            self.sign(&msg).await;
                        }
                        Command::Aggregate(commit) => {
                            self.aggregate(commit).await;
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
    async fn cubesat_stop() {
        let (result_tx, _) = mpsc::channel(1);
        let (request_tx, request_rx) = mpsc::channel(15);

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
        );

        tokio::spawn(async move {
            c.run().await;
        });

        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        request_tx
            .send(Command::Stop)
            .await
            .expect("Failed to send stop command");
    }

    #[tokio::test]
    async fn cubesat_sign() {
        let (result_tx, mut result_rx) = mpsc::channel(1);
        let (request_tx, request_rx) = mpsc::channel(15);

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
        );

        tokio::spawn(async move {
            c.run().await;
        });

        tokio::spawn(async move {
            request_tx
                .send(Command::Sign("hello".as_bytes().to_vec()))
                .await
                .expect("failed to send sign command");
        });

        let result_opt = result_rx.recv().await;
        assert!(result_opt.is_some());
        let cmd = result_opt.unwrap();
        assert!(matches!(cmd, Command::Aggregate { .. }));
    }
}
