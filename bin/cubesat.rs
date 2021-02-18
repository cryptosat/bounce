extern crate bitcoin_hashes;
extern crate secp256k1;

use bitcoin_hashes::{sha256, Hash};
use secp256k1::rand::rngs::OsRng;
use secp256k1::{
    Error, Message, PublicKey, Secp256k1, SecretKey, Signature, Signing, Verification,
};

fn verify<C: Verification>(
    secp: &Secp256k1<C>,
    msg: &[u8],
    sig: Signature,
    pubkey: PublicKey,
) -> Result<bool, Error> {
    let msg = sha256::Hash::hash(msg);
    let msg = Message::from_slice(&msg)?;
    Ok(secp.verify(&msg, &sig, &pubkey).is_ok())
}

fn sign<C: Signing>(
    secp: &Secp256k1<C>,
    msg: &[u8],
    seckey: &SecretKey,
) -> Result<Signature, Error> {
    let msg = sha256::Hash::hash(msg);
    let msg = Message::from_slice(&msg)?;
    Ok(secp.sign(&msg, &seckey))
}

fn main() {
    let secp = Secp256k1::new();
    let mut rng = OsRng::new().unwrap();
    // generate public and private key pairs.
    let (seckey, pubkey) = secp.generate_keypair(&mut rng);
    assert_eq!(pubkey, PublicKey::from_secret_key(&secp, &seckey));

    // Read message to sign
    println!("Enter your name: ");
    let mut name = String::new();
    std::io::stdin()
        .read_line(&mut name)
        .expect("Failed to read line");

    println!("Hello, {}!", &name[..name.len() - 1]);

    let sig = sign(&secp, name.as_bytes(), &seckey).unwrap();
    assert!(verify(&secp, name.as_bytes(), sig, pubkey).unwrap());
}
