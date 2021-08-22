extern crate bls_signatures_rs;
extern crate bn;
extern crate hex;
extern crate rand;

use bls_signatures_rs::bn256::Bn256;
use bls_signatures_rs::MultiSignature;
use rand::{thread_rng, Rng};

fn main() {
    // Inputs: Secret Key, Public Key (derived) & Message

    let mut rng = thread_rng();

    let secret_key_1: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    println!("Size of the secret key: {}", secret_key_1.len());
    let secret_key_2: Vec<u8> = (0..32).map(|_| rng.gen()).collect();

    // Derive public keys from secret key
    let public_key_1 = Bn256.derive_public_key(&secret_key_1).unwrap();
    let public_key_2 = Bn256.derive_public_key(&secret_key_2).unwrap();

    let message: &[u8] = b"sample";

    // Sign identical message with two different secret keys
    let sig_1 = Bn256.sign(&secret_key_1, &message).unwrap();
    println!("Size of the signature: {}", sig_1.len());
    let sig_2 = Bn256.sign(&secret_key_2, &message).unwrap();

    // Aggregate public keys
    let agg_pub_key = Bn256
        .aggregate_public_keys(&[&public_key_1, &public_key_2])
        .unwrap();

    // Aggregate signatures
    let agg_sig = Bn256.aggregate_signatures(&[&sig_1, &sig_2]).unwrap();

    // Check whether the aggregated signature corresponds to the aggregated public key
    let _ = Bn256.verify(&agg_sig, &message, &agg_pub_key).unwrap();
    println!("Successful verification");

    let secret_key_3: Vec<u8> = (0..32).map(|_| rng.gen()).collect();
    let public_key_3 = Bn256.derive_public_key(&secret_key_3).unwrap();
    let sig_3 = Bn256.sign(&secret_key_3, &message).unwrap();

    let agg_pub_key_2 = Bn256
        .aggregate_public_keys(&[&agg_pub_key, &public_key_3])
        .unwrap();
    let agg_sig_2 = Bn256.aggregate_signatures(&[&agg_sig, &sig_3]).unwrap();

    let _ = Bn256.verify(&agg_sig_2, &message, &agg_pub_key_2).unwrap();
    println!("Successful verification");
}
