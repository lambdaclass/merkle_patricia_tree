use std::{collections::BTreeMap, error::Error};

use patricia_merkle_tree::PatriciaMerkleTree;
use rand::{thread_rng, RngCore};
use sha3::Keccak256;

fn main() -> Result<(), Box<dyn Error>> {
    let n = std::env::args().nth(1).expect("missing number of nodes");
    let n: usize = n.parse()?;

    let mut data = BTreeMap::new();

    while data.len() < n {
        let mut rng = thread_rng();
        let mut key = [0u8; 32];
        let mut value = [0u8; 32];
        rng.fill_bytes(&mut key);
        rng.fill_bytes(&mut value);
        data.insert(key, value);
    }

    let data: Vec<_> = data.into_iter().collect();

    let hash = PatriciaMerkleTree::<[u8; 32], [u8; 32], Keccak256>::compute_hash_from_sorted_iter(
        data.iter(),
    );

    println!("{hash:x}");

    Ok(())
}
