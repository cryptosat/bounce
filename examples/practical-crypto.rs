use hex_literal::hex;
use sha3::{Digest, Sha3_256};

fn main() {
    let mut hasher = Sha3_256::new();
    hasher.update(b"hello");
    let result = hasher.finalize();
    assert_eq!(result[..], hex!("
        3338be694f50c5f338814986cdf0686453a888b84f424d792af4b9202398f392
    ")[..]);
}