extern crate openssl;

use openssl::hash::hash;
use openssl::hash::MessageDigest;
use openssl::pkey::PKey;
use openssl::rsa::Rsa;
use openssl::sign::Signer;
use openssl::sign::Verifier;

fn main() {
    let data0 = b"hello, world!";
    let data1 = b"hola, mundo!";
    println!("Data0 size in bytes: {}", std::mem::size_of_val(data0));
    println!("Data1 size in bytes: {}", std::mem::size_of_val(data1));

    // Generate a keypair
    let keypair = Rsa::generate(2048).unwrap();
    let keypair = PKey::from_rsa(keypair).unwrap();

    // Sign the data
    let mut signer = Signer::new(MessageDigest::sha256(), &keypair).unwrap();
    signer.update(data0).unwrap();
    // signer.update(data1).unwrap();
    let signature = signer.sign_to_vec().unwrap();
    println!("Signature bytes: {}", std::mem::size_of_val(&signature));

    // To verify the signature, we need the original data.
    let mut verifier = Verifier::new(MessageDigest::sha256(), &keypair).unwrap();
    verifier.update(data0).unwrap();
    assert!(verifier.verify(&signature).unwrap());

    let h = hash(MessageDigest::sha256(), data0).unwrap();
    println!("Size of hash in bytes: {}", std::mem::size_of_val(&*h));
}
