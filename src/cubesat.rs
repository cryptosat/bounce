use bls_signatures::{PrivateKey, PublicKey, Serialize};
use rand::thread_rng;

#[derive(Debug)]
pub struct Cubesat {
    private_key: PrivateKey,
    public_key: PublicKey,
}

impl Default for Cubesat {
    fn default() -> Self {
        let mut rng = thread_rng();

        let private_key = PrivateKey::generate(&mut rng);
        let public_key = private_key.public_key();
        // generate public and private key pairs.

        Cubesat {
            private_key,
            public_key,
        }
    }
}

impl Cubesat {
    pub fn new() -> Cubesat {
        Default::default()
    }

    pub fn sign(&self, sigs: &mut AggregateSignature) {
        if sigs.public_keys.contains(self.public_key) {
            return;
        }

        let sig = self.private_key.sign(&sigs.msg);

        match sigs.signature {
            None => {
                sigs.signature = Some(sig);
            }
            Some(aggregate_signature) {
                sigs.signature = Some(bls_signatures::aggregate(&[
                    aggregate_signature, sig
                ]));
            }
        }
        sigs.public_keys.push(self.public_key);
    }
}
