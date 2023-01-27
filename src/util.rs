use crate::{nibble::Nibble, Encode};
use digest::{Digest, Output};
use std::borrow::Cow;

pub fn compute_hash_from_sorted_iter<'a, P, V, H>(
    iter: impl IntoIterator<Item = (&'a P, &'a V)>,
) -> Output<H>
where
    P: 'a + Encode + Clone,
    V: 'a + Encode + Clone,
    H: Digest,
{
    let mut stack = Vec::<StackFrame<H>>::new();
    for (path, value) in iter {
        let path = path.encode();
        let value = value.encode();

        if stack.is_empty() {
            stack.push(StackFrame::new_leaf(path, value));
        } else {
            let prefix_len = loop {
                let last_frame = match stack.last() {
                    None => break 0,
                    Some(x) => x,
                };

                let prefix_len = last_frame.prefix.count_prefix_len(path.as_ref());
                if prefix_len == last_frame.prefix.len() {
                    break prefix_len;
                }

                let frame = stack.pop().unwrap();

                // TODO: Hash (leaf or branch).

                // TODO: Hash extension if necessary (common prefix).

                // TODO: Insert into parent (if any).
                if let Some(parent_frame) = stack.last() {
                    parent_frame.choices[frame.prefix.nth(parent_frame.prefix.len()) as usize] =
                        todo!();
                }
            };

            stack.push(StackFrame::new_leaf(path, value));
        }
    }

    todo!()
}

struct StackFrame<'a, H: Digest> {
    pub prefix: NibblePrefix<'a>,
    pub choices: [(Output<H>, usize); 16],
    pub value: Option<Cow<'a, [u8]>>,
}

impl<'a, H: Digest> StackFrame<'a, H> {
    pub fn new_leaf(path: Cow<'a, [u8]>, value: Cow<'a, [u8]>) -> Self {
        Self {
            prefix: NibblePrefix::new(path),
            choices: Default::default(),
            value: Some(value),
        }
    }
}

struct NibblePrefix<'a>(Cow<'a, [u8]>, Option<Nibble>);

impl<'a> NibblePrefix<'a> {
    pub fn new(data: Cow<'a, [u8]>) -> Self {
        Self(data, None)
    }

    pub fn len(&self) -> usize {
        2 * self.0.len() + self.1.is_some() as usize
    }

    pub fn count_prefix_len(&self, other: &[u8]) -> usize {
        let mut other_iter = other.iter();
        let mut count = self
            .0
            .iter()
            .zip(&mut other_iter)
            .take_while(|(a, b)| a == b)
            .count();

        count *= 2;
        if let (Some(a), Some(b)) = (self.1, other_iter.next()) {
            if a as u8 == b >> 4 {
                count += 1;
            }
        }

        count
    }

    pub fn nth(&self, index: usize) -> Nibble {
        match self.1 {
            Some(x) if index == 2 * self.0.len() => x,
            _ => Nibble::try_from(if index % 2 == 0 {
                self.0[index >> 1] >> 4
            } else {
                self.0[index >> 1] & 0x0F
            })
            .unwrap(),
        }
    }
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
