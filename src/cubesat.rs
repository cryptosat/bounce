use super::AggregateSignature;
use bls_signatures::{PrivateKey, PublicKey};
use rand::thread_rng;

#[derive(Debug)]
pub struct Cubesat {
    private_key: PrivateKey,
    public_key: PublicKey,
}

impl Default for Cubesat {
    fn default() -> Self {
        let mut rng = thread_rng();

        // generate public and private key pairs.
        let private_key = PrivateKey::generate(&mut rng);
        let public_key = private_key.public_key();

        println!("Cubesat with public_key: {:?}", public_key);

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
        let sig = self.private_key.sign(&sigs.msg);
        assert!(self.public_key.verify(sig, &sigs.msg));
        println!("Successfully signed the message {:?}", &sigs.msg);

        sigs.signatures.push(sig);
        sigs.public_keys.push(self.public_key);
    }
}
