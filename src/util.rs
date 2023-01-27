#![warn(warnings)]

use crate::{
    hashing::{DelimitedHash, NodeHash},
    nibble::{Nibble, NibbleSlice},
    nodes::{compute_branch_hash, compute_leaf_hash},
    Encode,
};
use digest::{Digest, Output};
use std::{borrow::Cow, fmt::Debug};

pub fn compute_hash_from_sorted_iter<'a, P, V, H>(
    iter: impl IntoIterator<Item = (&'a P, &'a V)>,
) -> Output<H>
where
    P: 'a + Encode,
    V: 'a + Encode,
    H: Digest + Debug,
{
    let mut stack = Vec::<StackFrame<H>>::new();
    let walk_stack = |stack: &mut Vec<StackFrame<_>>, target: Option<&[u8]>| loop {
        // Get the top frame from the stack.
        let last_frame = stack.last().unwrap();

        // If the matched prefix length is shorter or equal, then the correct frame has been found.
        let prefix_len = match target {
            Some(path) => {
                let prefix_len = last_frame.prefix.count_prefix_len(path.as_ref());
                if prefix_len == last_frame.prefix.len() {
                    break None;
                }
                Some(prefix_len)
            }
            None => None,
        };

        // At this point, just extract the frame since it won't be used anymore.
        let mut frame = stack.pop().unwrap();

        // Hash leaf or branch.
        let frame_hash = NodeHash::<H>::default();
        match (frame.choices, frame.value) {
            (Some(choices), value) => {
                compute_branch_hash(&frame_hash, &choices, value.as_deref());
            }
            (None, Some(value)) => {
                // Extract the entire prefix, then apply an offset within a NibbleSlice.
                // Since it's a leaf, it'll never end with a single nibble.
                let prefix = frame.prefix.into_bytes();
                let prefix_offset = match stack.last() {
                    Some(x) => x.prefix.len() + x.choices.is_some() as usize,
                    None => prefix_len.map(|x| x + 1).unwrap_or_default(),
                };

                let mut prefix_slice = NibbleSlice::new(prefix);
                prefix_slice.offset_add(prefix_offset);

                compute_leaf_hash(&frame_hash, prefix_slice, value.as_ref());
                println!("#### {prefix_offset} {prefix_len:?}");
                println!("#### {frame_hash:02x?}");
            }
            _ => unreachable!(),
        }

        // TODO: Add branch if necessary:
        //   - Has a target (or don't add branches when there's only a leaf).
        //   - Popped frame is a leaf.
        //   - Has no matching branch (TODO: at least no parent, but maybe when an extension is needed too?).
        match prefix_len {
            Some(prefix_len) if stack.is_empty() => {
                let index = frame.prefix.truncate_and_get(prefix_len) as usize;
                stack.push(StackFrame {
                    prefix: frame.prefix,
                    choices: Some({
                        let mut choices = <[DelimitedHash<H>; 16]>::default();
                        choices[index] = frame_hash.into();
                        choices
                    }),
                    value: None,
                });

                continue;
            }
            _ => {}
        }

        // TODO: Add extension if necessary.

        // TODO: Insert into parent (when not the root).
        if let Some(parent_frame) = stack.last_mut() {
            let choices = parent_frame.choices.get_or_insert_with(Default::default);
            let choice_index = frame.prefix.nth(parent_frame.prefix.len()) as usize;

            choices[choice_index] = frame_hash.into();
        } else {
            break Some(frame_hash.into_inner());
        }
    };

    for (path, value) in iter {
        let path = path.encode();
        let value = value.encode();

        if stack.is_empty() {
            stack.push(StackFrame::new_leaf(path, value));
        } else {
            walk_stack(&mut stack, Some(path.as_ref()));

            // TODO: Hash extension if necessary (common prefix).

            stack.push(StackFrame::new_leaf(path, value));
        }
    }

    if stack.is_empty() {
        H::new().chain_update([0x80]).finalize()
    } else {
        let (mut hash_data, hash_len) = walk_stack(&mut stack, None).unwrap();
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
struct NibblePrefix<'a>(Cow<'a, [u8]>, Option<Nibble>);

impl<'a> NibblePrefix<'a> {
    pub fn new(data: Cow<'a, [u8]>) -> Self {
        Self(data, None)
    }

    pub fn len(&self) -> usize {
        2 * self.0.len() + self.1.is_some() as usize
    }

    pub fn truncate_and_get(&mut self, prefix_len: usize) -> Nibble {
        let next_nibble = if prefix_len % 2 == 0 {
            if (prefix_len >> 1) == self.0.len() {
                self.1.unwrap()
            } else {
                Nibble::try_from(self.0[prefix_len >> 1] >> 4).unwrap()
            }
        } else {
            Nibble::try_from(self.0[prefix_len >> 1] & 0x0F).unwrap()
        };

        match &mut self.0 {
            Cow::Borrowed(x) => *x = &x[..prefix_len >> 1],
            Cow::Owned(x) => x.truncate(prefix_len >> 1),
        }

        next_nibble
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

    pub fn into_bytes(&self) -> &[u8] {
        assert!(self.1.is_none());
        self.0.as_ref()
    }
}

#[cfg(test)]
mod test {
    use super::compute_hash_from_sorted_iter;
    use crate::PatriciaMerkleTree;
    use proptest::{
        collection::{btree_set, vec},
        prelude::*,
    };
    use sha3::Keccak256;
    use std::sync::Arc;

    #[test]
    fn test_empty_tree() {
        const DATA: &[(&[u8], &[u8])] = &[];

        let computed_hash =
            compute_hash_from_sorted_iter::<_, _, Keccak256>(DATA.iter().map(|(a, b)| (a, b)));
        let expected_hash =
            compute_hash_cita_trie(DATA.iter().map(|(a, b)| (a.to_vec(), b.to_vec())).collect());

        assert_eq!(computed_hash.as_slice(), expected_hash.as_slice());
    }

    #[test]
    fn test_leaf_tree() {
        const DATA: &[(&[u8], &[u8])] = &[(b"hello", b"world")];

        let computed_hash =
            compute_hash_from_sorted_iter::<_, _, Keccak256>(DATA.iter().map(|(a, b)| (a, b)));
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

        let computed_hash =
            compute_hash_from_sorted_iter::<_, _, Keccak256>(DATA.iter().map(|(a, b)| (a, b)));
        let expected_hash =
            compute_hash_cita_trie(DATA.iter().map(|(a, b)| (a.to_vec(), b.to_vec())).collect());

        assert_eq!(computed_hash.as_slice(), expected_hash.as_slice());
    }

    // proptest! {
    //     #[test]
    //     fn proptest_compare_hashes_simple(path in vec(any::<u8>(), 1..32), value in vec(any::<u8>(), 1..100)) {
    //         expect_hash(vec![(path, value)])?;
    //     }

    //     #[test]
    //     fn proptest_compare_hashes_multiple(data in btree_set((vec(any::<u8>(), 1..32), vec(any::<u8>(), 1..100)), 1..100)) {
    //         expect_hash(data.into_iter().collect())?;
    //     }
    // }

    // fn expect_hash(data: Vec<(Vec<u8>, Vec<u8>)>) -> Result<(), TestCaseError> {
    //     prop_assert_eq!(
    //         compute_hash_cita_trie(data.clone()),
    //         compute_hash_ours(data)
    //     );
    //     Ok(())
    // }

    // fn compute_hash_ours(data: Vec<(Vec<u8>, Vec<u8>)>) -> Vec<u8> {
    //     PatriciaMerkleTree::<_, _, Keccak256>::compute_hash_from_sorted_iter(
    //         data.iter().map(|(a, b)| (a, b)),
    //     )
    //     .to_vec()
    // }

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
