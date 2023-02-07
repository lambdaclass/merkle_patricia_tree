use crate::{
    hashing::{DelimitedHash, NodeHash},
    nibble::{Nibble, NibbleSlice},
    nodes::{compute_branch_hash, compute_extension_hash, compute_leaf_hash},
    Encode,
};
use digest::{Digest, Output};
use std::{borrow::Cow, cmp::max, fmt::Debug};

pub fn compute_hash_from_sorted_iter<'a, P, V, H>(
    iter: impl IntoIterator<Item = &'a (P, V)>,
) -> Output<H>
where
    P: 'a + Encode,
    V: 'a + Encode,
    H: Digest,
{
    let mut stack = Vec::<StackFrame<H>>::new();

    let hash_frame = |frame: &StackFrame<_>, offset_delta: usize| {
        let hash = NodeHash::default();
        match (&frame.choices, &frame.value) {
            (Some(choices), value) => {
                if frame.prefix.len() > offset_delta {
                    let child_hash = NodeHash::default();
                    let child_hash_ref = compute_branch_hash::<DelimitedHash<H>, H>(
                        &child_hash,
                        choices,
                        value.as_deref(),
                    );

                    let mut path = NibbleSlice::new(&frame.prefix.0);
                    path.offset_add(offset_delta);

                    let prefix = path.split_to_vec(frame.prefix.len() - offset_delta);
                    compute_extension_hash(&hash, &prefix, child_hash_ref);
                } else {
                    compute_branch_hash::<DelimitedHash<H>, H>(&hash, choices, value.as_deref());
                }
            }
            (None, Some(value)) => {
                compute_leaf_hash::<H>(
                    &hash,
                    {
                        let mut path = NibbleSlice::new(frame.prefix.as_bytes());
                        path.offset_add(offset_delta);
                        path
                    },
                    value.as_ref(),
                );
            }
            (None, None) => unreachable!(),
        }

        hash
    };

    let pop_and_hash = |stack: &mut Vec<StackFrame<_>>, target_len: usize| {
        let mut popped_frame = stack.pop().unwrap();

        match stack.last_mut() {
            Some(top_frame) => {
                let hash = hash_frame(&popped_frame, target_len + 1);

                if top_frame.prefix.len() == target_len {
                    let choices = top_frame.choices.get_or_insert_with(Default::default);
                    choices[popped_frame.prefix.get_nth(target_len) as usize] = hash.into();
                } else {
                    let next_nibble = popped_frame.prefix.get_nth(target_len);
                    let branch_choices = {
                        let mut choices = <[DelimitedHash<H>; 16]>::default();
                        choices[next_nibble as usize] =
                            hash_frame(&popped_frame, target_len + 1).into();
                        choices
                    };

                    popped_frame.prefix.truncate(target_len);
                    let branch_frame = StackFrame {
                        prefix: popped_frame.prefix,
                        choices: Some(branch_choices),
                        value: None,
                    };

                    stack.push(branch_frame);
                }
            }
            None => {
                assert_ne!(popped_frame.prefix.len(), 0);

                let next_nibble = popped_frame.prefix.get_nth(target_len);
                let branch_choices = {
                    let mut choices = <[DelimitedHash<H>; 16]>::default();
                    choices[next_nibble as usize] =
                        hash_frame(&popped_frame, target_len + 1).into();
                    choices
                };

                popped_frame.prefix.truncate(target_len);
                let branch_frame = StackFrame {
                    prefix: popped_frame.prefix,
                    choices: Some(branch_choices),
                    value: None,
                };

                stack.push(branch_frame);
            }
        }
    };

    let pop_until_target = |stack: &mut Vec<StackFrame<_>>, target: &[u8]| loop {
        let top_frame = stack.last().unwrap();
        let common_prefix_len = top_frame.prefix.count_prefix_len(target);

        if common_prefix_len == top_frame.prefix.len() {
            break;
        }

        pop_and_hash(
            stack,
            if stack.len() < 2 {
                common_prefix_len
            } else {
                max(common_prefix_len, stack[stack.len() - 2].prefix.len())
            },
        );
    };

    for (path, value) in iter {
        let path = path.encode();
        let value = value.encode();

        if let Some(_top_frame) = stack.last() {
            // TODO: Assert that path > top_frame.prefix.
            pop_until_target(&mut stack, path.as_ref());
        }
        stack.push(StackFrame::new_leaf(path, value));
    }

    if stack.is_empty() {
        H::new().chain_update([0x80]).finalize()
    } else {
        while stack.len() > 1 {
            let target_len = stack[stack.len() - 2].prefix.len();
            pop_and_hash(&mut stack, target_len);
        }

        let (mut hash_data, hash_len) = hash_frame(&stack[0], 0).into_inner();
        if hash_len < 32 {
            H::new()
                .chain_update(&hash_data[..hash_len])
                .finalize_into(&mut hash_data);
        }

        hash_data
    }
}

#[derive(Debug)]
struct StackFrame<'a, H>
where
    H: Digest,
{
    pub prefix: NibblePrefix<'a>,
    pub choices: Option<[DelimitedHash<H>; 16]>,
    pub value: Option<Cow<'a, [u8]>>,
}

impl<'a, H> StackFrame<'a, H>
where
    H: Digest,
{
    pub fn new_leaf(path: Cow<'a, [u8]>, value: Cow<'a, [u8]>) -> Self {
        Self {
            prefix: NibblePrefix::new(path),
            choices: Default::default(),
            value: Some(value),
        }
    }
}

#[derive(Debug)]
struct NibblePrefix<'a>(Cow<'a, [u8]>, bool);

impl<'a> NibblePrefix<'a> {
    pub fn new(data: Cow<'a, [u8]>) -> Self {
        Self(data, false)
    }

    pub fn len(&self) -> usize {
        2 * self.0.len() - self.1 as usize
    }

    pub fn get_nth(&self, index: usize) -> Nibble {
        Nibble::try_from(if index % 2 == 0 {
            self.0[index >> 1] >> 4
        } else {
            // Check out of bounds when ending in half-byte.
            if (index >> 1) + 1 == self.0.len() && self.1 {
                panic!("out of range")
            } else {
                self.0[index >> 1] & 0x0F
            }
        })
        .unwrap()
    }

    pub fn truncate(&mut self, prefix_len: usize) {
        self.1 = prefix_len % 2 != 0;
        match &mut self.0 {
            Cow::Borrowed(x) => *x = &x[..(prefix_len + 1) >> 1],
            Cow::Owned(x) => x.truncate((prefix_len + 1) >> 1),
        }
    }

    pub fn count_prefix_len(&self, other: &[u8]) -> usize {
        let count = self
            .0
            .iter()
            .take(self.len() - self.1 as usize)
            .zip(other.iter())
            .take_while(|(a, b)| a == b)
            .count();

        if let (Some(a), Some(b)) = (self.0.get(count), other.get(count)) {
            if a >> 4 == b >> 4 {
                return 2 * count + 1;
            }
        }

        2 * count
    }

    pub fn as_bytes(&self) -> &[u8] {
        assert!(!self.1);
        self.0.as_ref()
    }
}

#[cfg(test)]
mod test {
    use super::compute_hash_from_sorted_iter;
    use crate::PatriciaMerkleTree;
    use proptest::{
        collection::{btree_map, vec},
        prelude::*,
    };
    use sha3::Keccak256;
    use std::sync::Arc;

    #[test]
    fn test_empty_tree() {
        const DATA: &[(&[u8], &[u8])] = &[];

        let computed_hash = compute_hash_from_sorted_iter::<_, _, Keccak256>(DATA.iter());
        let expected_hash =
            compute_hash_cita_trie(DATA.iter().map(|(a, b)| (a.to_vec(), b.to_vec())).collect());

        assert_eq!(computed_hash.as_slice(), expected_hash.as_slice());
    }

    #[test]
    fn test_leaf_tree() {
        const DATA: &[(&[u8], &[u8])] = &[(b"hello", b"world")];

        let computed_hash = compute_hash_from_sorted_iter::<_, _, Keccak256>(DATA.iter());
        let expected_hash =
            compute_hash_cita_trie(DATA.iter().map(|(a, b)| (a.to_vec(), b.to_vec())).collect());

        assert_eq!(computed_hash.as_slice(), expected_hash.as_slice());
    }

    #[test]
    fn test_branch_tree() {
        const DATA: &[(&[u8], &[u8])] = &[
            (&[0x00], &[0x00]),
            (&[0x10], &[0x10]),
            (&[0x20], &[0x20]),
            (&[0x30], &[0x30]),
        ];

        let computed_hash = compute_hash_from_sorted_iter::<_, _, Keccak256>(DATA.iter());
        let expected_hash =
            compute_hash_cita_trie(DATA.iter().map(|(a, b)| (a.to_vec(), b.to_vec())).collect());

        assert_eq!(computed_hash.as_slice(), expected_hash.as_slice());
    }

    #[test]
    fn test_extension_tree() {
        const DATA: &[(&[u8], &[u8])] = &[
            (&[0x00], &[0x00]),
            (&[0x01], &[0x01]),
            (&[0x02], &[0x02]),
            (&[0x03], &[0x03]),
        ];

        let computed_hash = compute_hash_from_sorted_iter::<_, _, Keccak256>(DATA.iter());
        let expected_hash =
            compute_hash_cita_trie(DATA.iter().map(|(a, b)| (a.to_vec(), b.to_vec())).collect());

        assert_eq!(computed_hash.as_slice(), expected_hash.as_slice());
    }

    proptest! {
        #[test]
        fn proptest_compare_hashes_simple(path in vec(any::<u8>(), 1..32), value in vec(any::<u8>(), 1..100)) {
            expect_hash(vec![(path, value)])?;
        }

        #[test]
        fn proptest_compare_hashes_multiple(data in btree_map(vec(any::<u8>(), 1..32), vec(any::<u8>(), 1..100), 1..100)) {
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
        PatriciaMerkleTree::<_, _, Keccak256>::compute_hash_from_sorted_iter(data.iter()).to_vec()
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
