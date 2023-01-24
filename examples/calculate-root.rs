use patricia_merkle_tree::PatriciaMerkleTree;
use sha3::Keccak256;

fn main() {
    let mut tree = PatriciaMerkleTree::<Vec<u8>, Vec<u8>, Keccak256>::new();

    for x in 1..100 {
        tree.insert(
            (0..x).into_iter().collect(),
            (0..x).into_iter().cycle().take(100).collect(),
        );
    }

    let hash = tree.compute_hash();

    println!("{hash:x}");
}
