#![warn(warnings)]

use crate::{
    hashing::{DelimitedHash, NodeHash},
    nibble::{Nibble, NibbleSlice},
    nodes::{compute_branch_hash, compute_extension_hash, compute_leaf_hash},
    Encode,
};
use digest::{Digest, Output};
use std::{borrow::Cow, cmp::max, fmt::Debug};

pub fn compute_hash_from_sorted_iter<'a, P, V, H>(
    iter: impl IntoIterator<Item = (&'a P, &'a V)>,
) -> Output<H>
where
    P: 'a + Encode,
    V: 'a + Encode,
    H: Digest + Debug,
{
    let mut stack = Vec::<StackFrame<H>>::new();

    let hash_frame = |frame: &StackFrame<_>, offset_delta: usize| {
        let hash = NodeHash::default();
        // dbg!((&frame.choices, &frame.value, offset_delta));
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
                // TODO: popped_frame.prefix.get_nth(top_frame.prefix.len()) -> maybe extension -> into choices -> insert into top_frame.
                // let child_ref = popped_frame.prefix.get_nth(top_frame.prefix.len());

                let hash = hash_frame(&popped_frame, target_len + 1);
                // println!("insert into top_frame or add extra branch? {}", if top_frame.prefix.len() == target_len { "insert" } else { "extra branch" });

                if top_frame.prefix.len() == target_len {
                    let choices = top_frame.choices.get_or_insert_with(Default::default);
                    choices[popped_frame.prefix.get_nth(target_len) as usize] = hash.into();
                    // todo!("insert only (maybe extension)");
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
                        offset: target_len,
                        prefix: popped_frame.prefix,
                        choices: Some(branch_choices),
                        value: None,
                    };

                    stack.push(branch_frame);
                    // todo!(
                    //     "aux branch with child branch (maybe extension between child and auxiliar)"
                    // );
                }

                // todo!("branch + optional extension, then insert?")
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
                    offset: target_len,
                    prefix: popped_frame.prefix,
                    choices: Some(branch_choices),
                    value: None,
                };

                stack.push(branch_frame);
            }
        }

        // todo!()
    };

    let pop_until_target = |stack: &mut Vec<StackFrame<_>>, target: &[u8]| loop {
        // println!("stack  = {stack:02x?}");
        // println!("target = {target:02x?}");
        // println!();

        // len(common_prefix(target, top_frame.prefix)) > len(top_frame.prefix) -> unreachable (literally not possible)
        // len(common_prefix(target, top_frame.prefix)) = len(top_frame.prefix) -> done (len(path) must be > than len(prefix) due to assert)
        // len(common_prefix(target, top_frame.prefix)) < len(top_frame.prefix) -> pop
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
        dump_stack(&stack);
    }

    println!("## Popping everything from the stack.");
    if stack.is_empty() {
        H::new().chain_update([0x80]).finalize()
    } else {
        // let (mut hash_data, hash_len) = pop_until_target(&mut stack, None).unwrap();
        // if hash_len < 32 {
        //     H::new()
        //         .chain_update(&hash_data[..hash_len])
        //         .finalize_into(&mut hash_data);
        // }

        // hash_data

        while stack.len() > 1 {
            let target_len = stack[stack.len() - 2].prefix.len();
            pop_and_hash(&mut stack, target_len);
            dump_stack(&stack);
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

fn dump_stack<H>(stack: &[StackFrame<H>])
where
    H: Digest,
{
    println!("Stack:");
    for (idx, frame) in stack.iter().enumerate() {
        println!(
            "  [{idx:2}] prefix: {}, choices: {{{}}}, value: {:?}",
            match frame.prefix.len() {
                0 => "<nil>".to_string(),
                _ => (0..frame.prefix.len())
                    .map(|i| format!("{:x}", frame.prefix.get_nth(i) as usize))
                    .collect::<String>(),
            },
            frame
                .choices
                .as_ref()
                .map(|x| x
                    .iter()
                    .enumerate()
                    .filter_map(|(i, x)| if x.1 == 0 {
                        None
                    } else {
                        Some(format!("{i:x}: {:02x?}", &x.0[..x.1]))
                    })
                    .intersperse_with(|| ", ".to_string())
                    .collect::<String>())
                .unwrap_or_default(),
            frame.value
        );
    }
    println!();
}

#[derive(Debug)]
struct StackFrame<'a, H>
where
    H: Digest,
{
    pub offset: usize,
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
            offset: 2 * path.len(),
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
            .take(self.len() - if self.1 { 1 } else { 0 })
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

    pub fn nth(&self, index: usize) -> Nibble {
        if self.1 && index == 2 * self.0.len() - 1 {
            panic!("out of bounds");
        }

        Nibble::try_from(if index % 2 == 0 {
            self.0[index >> 1] >> 4
        } else {
            self.0[index >> 1] & 0x0F
        })
        .unwrap()
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
    fn test_asdf() {
        const DATA: &[(&[u8], &[u8])] = &[
            (&[0x00], &[0x00]),
            (&[0xB0], &[0x01]),
            (&[0xB1], &[0x02]), //
        ];

        let a = compute_hash_from_sorted_iter::<_, _, Keccak256>(DATA.iter().map(|(a, b)| (a, b)));
        let b =
            compute_hash_cita_trie(DATA.iter().map(|(a, b)| (a.to_vec(), b.to_vec())).collect());
        assert_eq!(a.as_slice(), b.as_slice());
    }

    #[test]
    fn test_asdf2() {
        const DATA: &[(&[u8], &[u8])] = &[
            (&[0x00], &[0x00]),
            (&[0xB6], &[0x01]),
            (&[0xB6, 0x00], &[0x02]), //
        ];

        let a = compute_hash_from_sorted_iter::<_, _, Keccak256>(DATA.iter().map(|(a, b)| (a, b)));
        let b =
            compute_hash_cita_trie(DATA.iter().map(|(a, b)| (a.to_vec(), b.to_vec())).collect());
        assert_eq!(a.as_slice(), b.as_slice());
    }

    #[test]
    fn test_asdf3() {
        const DATA: &[(&[u8], &[u8])] = &[
            (&[0x00], &[0x00]),
            (&[0xB6, 0x00], &[0x01]),
            (&[0xB6, 0x01], &[0x02]), //
        ];

        let a = compute_hash_from_sorted_iter::<_, _, Keccak256>(DATA.iter().map(|(a, b)| (a, b)));
        let b =
            compute_hash_cita_trie(DATA.iter().map(|(a, b)| (a.to_vec(), b.to_vec())).collect());
        assert_eq!(a.as_slice(), b.as_slice());
    }

    #[test]
    fn test_asdf4() {
        const DATA: &[(&[u8], &[u8])] = &[
            (&[0xB0], &[0x00]),
            (&[0xB1], &[0x01]),
            (&[0xC0], &[0x02]), //
        ];

        let a = compute_hash_from_sorted_iter::<_, _, Keccak256>(DATA.iter().map(|(a, b)| (a, b)));
        let b =
            compute_hash_cita_trie(DATA.iter().map(|(a, b)| (a.to_vec(), b.to_vec())).collect());
        assert_eq!(a.as_slice(), b.as_slice());
    }

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

    #[test]
    fn test_extension_tree() {
        const DATA: &[(&[u8], &[u8])] = &[
            (&[0x00], &[0x00]),
            (&[0x01], &[0x01]),
            (&[0x02], &[0x02]),
            (&[0x03], &[0x03]),
        ];

        let computed_hash =
            compute_hash_from_sorted_iter::<_, _, Keccak256>(DATA.iter().map(|(a, b)| (a, b)));
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

// let walk_stack = |stack: &mut Vec<StackFrame<_>>, target: Option<&[u8]>| loop {
//     // Get the top frame from the stack.
//     let last_frame = stack.last().unwrap();

//     // If the matched prefix length is shorter or equal, then the correct frame has been found.
//     let prefix_len = match target {
//         Some(path) => {
//             let prefix_len = last_frame.prefix.count_prefix_len(path.as_ref());
//             if prefix_len == last_frame.offset {
//                 // break None;
//                 Some(2 * path.len())
//             }else {
//             dbg!((prefix_len, last_frame.offset, &last_frame.prefix));
//             Some(prefix_len)}
//         }
//         None => None,
//     };
//     dbg!(prefix_len);

//     // At this point, just extract the frame since it won't be used anymore.
//     let mut frame = stack.pop().unwrap();

//     // Hash leaf or branch.
//     let frame_hash = NodeHash::<H>::default();
//     let is_branch = match (frame.choices, frame.value) {
//         (Some(choices), value) => {
//             compute_branch_hash(&frame_hash, &choices, value.as_deref());
//             true
//         }
//         (None, Some(value)) => {
//             // Extract the entire prefix, then apply an offset within a NibbleSlice.
//             // Since it's a leaf, it'll never end with a single nibble.
//             let prefix = frame.prefix.as_bytes();
//             let prefix_offset = match stack.last() {
//                 Some(x) => {
//                     x.offset
//                         + match x.choices {
//                             Some(_) => {
//                                 1 + (prefix_len.unwrap_or(0) > stack.last().unwrap().offset)
//                                     as usize
//                             }
//                             None => 1,
//                         }
//                 }
//                 None => prefix_len.map(|x| x + 1).unwrap_or_default(),
//             };
//             // println!(
//             //     "value = {value:?}; prefix_offset = {prefix_offset} ({})",
//             //     match (stack.last(), prefix_len) {
//             //         (Some(x), Some(prefix_len)) =>
//             //             x.choices.is_some() && prefix_len > stack.last().unwrap().offset,
//             //         _ => stack.is_empty(),
//             //     },
//             // );

//             let mut prefix_slice = NibbleSlice::new(prefix);
//             prefix_slice.offset_add(prefix_offset);

//             compute_leaf_hash(&frame_hash, prefix_slice, value.as_ref());
//             false
//         }
//         _ => unreachable!(),
//     };

//     // Add branch if necessary:
//     //   - Has a target (or don't add branches when there's only a leaf).
//     //   - Popped frame is a leaf.
//     //   - Has no matching branch (TODO: at least no parent, but maybe when an extension is needed too?).
//     match prefix_len {
//         Some(prefix_len) if stack.is_empty() || prefix_len > stack.last().unwrap().offset => {
//             let index = frame.prefix.truncate_and_get(prefix_len) as usize;
//             stack.push(StackFrame {
//                 offset: prefix_len + is_branch as usize,
//                 prefix: frame.prefix,
//                 choices: Some({
//                     let mut choices = <[DelimitedHash<H>; 16]>::default();
//                     choices[index] = frame_hash.into();
//                     choices
//                 }),
//                 value: None,
//             });

//             continue;
//         }
//         _ => {}
//     }

//     // Insert into parent (when not the root).
//     // TODO: Maybe improve this piece of code?
//     if let Some(parent_frame) = stack.last_mut() {
//         // let prefix_len = min(frame.offset, parent_frame.offset);
//         let prefix_offset = parent_frame.offset + parent_frame.choices.is_some() as usize;
//         let prefix_len = frame.offset - is_branch as usize + 1 - prefix_offset;

//         let choices = parent_frame.choices.get_or_insert_with(Default::default);
//         let choice_index = frame.prefix.nth(parent_frame.offset) as usize;

//         if is_branch && prefix_len != 0 {
//             let prefix = {
//                 let mut x = NibbleSlice::new(frame.prefix.0.as_ref());
//                 x.offset_add(prefix_offset);
//                 x.split_to_vec(prefix_len)
//             };

//             let extension_hash = NodeHash::default();
//             compute_extension_hash(&extension_hash, &prefix, frame_hash.extract_ref().unwrap());
//             choices[choice_index] = extension_hash.into();
//         } else {
//             choices[choice_index] = frame_hash.into();
//         }
//     } else {
//         break if is_branch && frame.prefix.len() != 0 {
//             let prefix =
//                 { NibbleSlice::new(frame.prefix.0.as_ref()).split_to_vec(frame.offset) };

//             let extension_hash = NodeHash::default();
//             compute_extension_hash(&extension_hash, &prefix, frame_hash.extract_ref().unwrap());
//             Some(extension_hash.into_inner())
//         } else {
//             Some(frame_hash.into_inner())
//         };
//     }
// };
