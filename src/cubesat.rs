use super::AggregateSignature;
use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use rand::{thread_rng, Rng};

pub struct Cubesat {
    private_key: Vec<u8>,
    public_key: Vec<u8>,
}

impl Default for Cubesat {
    fn default() -> Self {
        let mut rng = thread_rng();

        // generate public and private key pairs.
        let private_key: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
        let public_key = Bn256.derive_public_key(&private_key).unwrap();

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
        let signature = Bn256.sign(&self.private_key, &sigs.msg).unwrap();
        Bn256
            .verify(&signature, &sigs.msg, &self.public_key)
            .unwrap();
        println!("Successfully signed the message {:?}", &sigs.msg);

        sigs.signatures.push(signature);
        sigs.public_keys.push(self.public_key.clone());
    }
}
