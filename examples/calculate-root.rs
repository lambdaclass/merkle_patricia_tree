use std::error::Error;

use patricia_merkle_tree::PatriciaMerkleTree;
use rand::{thread_rng, RngCore};
use sha3::Keccak256;

fn main() -> Result<(), Box<dyn Error>> {
    let n = std::env::args().nth(1).expect("missing number of nodes");
    let n: usize = n.parse()?;
    let mut tree = PatriciaMerkleTree::<_, _, Keccak256>::new();

    let mut rng = thread_rng();
    let mut key = [0u8; 32];
    let mut value = [0u8; 32];

    for _ in 0..n {
        rng.fill_bytes(&mut key);
        rng.fill_bytes(&mut value);
        tree.insert(key, value);
    }

    let hash = tree.compute_hash();

    println!("{hash:x}");

    Ok(())
}
