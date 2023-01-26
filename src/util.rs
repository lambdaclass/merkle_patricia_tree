use crate::PatriciaMerkleTree;
use digest::{Digest, Output};

pub fn compute_hash_from_sorted_iter<'a, P, V, H>(
    iter: impl IntoIterator<Item = (&'a P, &'a V)>,
) -> Output<H>
where
    P: 'a + AsRef<[u8]> + Clone,
    V: 'a + AsRef<[u8]> + Clone,
    H: Digest,
{
    let mut tree = PatriciaMerkleTree::<P, V, H>::new();

    for (path, value) in iter {
        tree.insert(path.clone(), value.clone());
    }

    tree.compute_hash().clone()
}

#[cfg(test)]
mod test {
    use crate::PatriciaMerkleTree;
    use proptest::{
        collection::{btree_set, vec},
        prelude::*,
    };
    use sha3::Keccak256;
    use std::sync::Arc;

    proptest! {
        #[test]
        fn proptest_compare_hashes_simple(path in vec(any::<u8>(), 1..32), value in vec(any::<u8>(), 1..100)) {
            expect_hash(vec![(path, value)])?;
        }

        #[test]
        fn proptest_compare_hashes_multiple(data in btree_set((vec(any::<u8>(), 1..32), vec(any::<u8>(), 1..100)), 1..100)) {
            expect_hash(data.into_iter().collect())?;
        }
    }

    fn expect_hash(data: Vec<(Vec<u8>, Vec<u8>)>) -> Result<(), TestCaseError> {
        prop_assert_eq!(
            compute_hash_cita_trie(data.clone()),
            compute_hash_ours(data)
        );
        Ok(())
    }

    fn compute_hash_ours(data: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<u8> {
        PatriciaMerkleTree::<_, _, Keccak256>::compute_hash_from_sorted_iter(
            data.iter().map(|(a, b)| (a, b)),
        )
        .to_vec()
    }

    fn compute_hash_cita_trie(data: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<u8> {
        use cita_trie::MemoryDB;
        use cita_trie::{PatriciaTrie, Trie};
        use hasher::HasherKeccak;

        let memdb = Arc::new(MemoryDB::new(true));
        let hasher = Arc::new(HasherKeccak::new());

        let mut trie = PatriciaTrie::new(Arc::clone(&memdb), Arc::clone(&hasher));

        for (key, value) in data {
            trie.insert(key.to_vec(), value.to_vec()).unwrap();
        }

        trie.root().unwrap()
    }
}
