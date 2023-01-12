use super::{BranchNode, ExtensionNode};
use crate::{
    nibble::NibbleSlice,
    node::{InsertAction, Node},
    util::INVALID_REF,
    NodesStorage, ValuesStorage,
};
use digest::{Digest, Output};
use std::marker::PhantomData;

#[derive(Clone, Debug)]
pub struct LeafNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    value_ref: usize,

    hash: (usize, Output<H>),
    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> LeafNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    pub fn new(value_ref: usize) -> Self {
        Self {
            value_ref,
            hash: (0, Default::default()),
            phantom: PhantomData,
        }
    }

    pub fn update_value_ref(&mut self, new_value_ref: usize) {
        self.value_ref = new_value_ref;
    }

    pub fn get<'a>(
        &self,
        _nodes: &NodesStorage<P, V, H>,
        values: &'a ValuesStorage<P, V>,
        path: NibbleSlice,
    ) -> Option<&'a V> {
        // If the remaining path (and offset) matches with the value's path, return the value.
        // Otherwise, no value is present.

        let (value_path, value) = values
            .get(self.value_ref)
            .expect("inconsistent internal tree structure");

        path.cmp_rest(value_path.as_ref()).then_some(value)
    }

    pub fn insert(
        mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        path: NibbleSlice,
    ) -> (Node<P, V, H>, InsertAction) {
        // [x] leaf { key => value } -> leaf { key => value }
        // [ ] leaf { key => value } -> branch { 0 => leaf { key => value }, 1 => leaf { key => value } }
        // [ ] leaf { key => value } -> extension { [0], branch { 0 => leaf { key => value }, 1 => leaf { key => value } } }
        // [ ] leaf { key => value } -> extension { [0], branch { 0 => leaf { key => value } } with_value leaf { key => value } }
        // [ ] leaf { key => value } -> extension { [0], branch { 0 => leaf { key => value } } with_value leaf { key => value } } // leafs swapped

        self.hash.0 = 0;

        let (value_path, _) = values
            .get(self.value_ref)
            .expect("inconsistent internal tree structure");

        if path.cmp_rest(value_path.as_ref()) {
            let value_ref = self.value_ref;
            (self.into(), InsertAction::Replace(value_ref))
        } else {
            // TODO: Implement dedicated method (half-byte avoid iterators).
            let offset = NibbleSlice::new(value_path.as_ref())
                .zip(path.clone())
                .take_while(|(a, b)| a == b)
                .count();

            let (branch_node, mut insert_action) =
                if offset == path.as_ref().len() {
                    (
                        BranchNode::new({
                            let mut choices = [None; 16];
                            // TODO: Dedicated method.
                            choices[NibbleSlice::new(value_path.as_ref()).nth(offset).unwrap()
                                as usize] = Some(nodes.insert(self.into()));
                            choices
                        }),
                        InsertAction::InsertSelf,
                    )
                } else if offset == value_path.as_ref().len() {
                    let child_ref = nodes.insert(LeafNode::new(INVALID_REF).into());

                    (
                        BranchNode::new({
                            let mut choices = [None; 16];
                            // TODO: Dedicated method.
                            choices[NibbleSlice::new(value_path.as_ref()).nth(offset).unwrap()
                                as usize] = Some(child_ref);
                            choices
                        }),
                        InsertAction::Insert(child_ref),
                    )
                } else {
                    let child_ref = nodes.insert(LeafNode::new(INVALID_REF).into());

                    (
                        BranchNode::new({
                            let mut choices = [None; 16];
                            // TODO: Dedicated method.
                            choices[NibbleSlice::new(value_path.as_ref()).nth(offset).unwrap()
                                as usize] = Some(nodes.insert(self.into()));
                            // TODO: Dedicated method.
                            choices[NibbleSlice::new(value_path.as_ref()).nth(offset).unwrap()
                                as usize] = Some(child_ref);
                            choices
                        }),
                        InsertAction::Insert(child_ref),
                    )
                };

            let final_node = if offset != 0 {
                let branch_ref = nodes.insert(branch_node.into());
                insert_action = insert_action.quantize_self(branch_ref);

                ExtensionNode::new(path.split_to_vec(offset), branch_ref).into()
            } else {
                branch_node.into()
            };

            (final_node, insert_action)
        }
    }

    // pub fn remove<I>(
    //     self,
    //     _nodes: &mut NodesStorage<P, V, H>,
    //     values: &mut ValuesStorage<P, V>,
    //     path_iter: Offseted<I>,
    // ) -> (Option<Node<P, V, H>>, Option<V>)
    // where
    //     I: Iterator<Item = Nibble>,
    // {
    //     // Retrieve the value storage to compare paths and return the value if there's a match.
    //     let (value_path, _) = values
    //         .get(self.value_ref)
    //         .expect("inconsistent internal tree structure");

    //     // Compare remaining paths since everything before that should already be equal, then return
    //     // the value if they match.
    //     let path_offset = path_iter.offset();
    //     path_iter
    //         .eq(value_path.encoded_iter().skip(path_offset))
    //         .then(|| (None, Some(values.remove(self.value_ref).1)))
    //         .unwrap_or((Some(self.into()), None))
    // }

    // pub fn compute_hash(
    //     &mut self,
    //     _nodes: &mut NodesStorage<P, V, H>,
    //     values: &ValuesStorage<P, V>,
    //     key_offset: usize,
    // ) -> &[u8] {
    //     if self.hash.0 == 0 {
    //         let (key, value) = values
    //             .get(self.value_ref)
    //             .expect("inconsistent internal tree structure");

    //         let mut digest_buf = DigestBuf::<H>::new();

    //         // Encode key.
    //         // TODO: Improve performance by avoiding allocations.
    //         let key: Vec<_> = key.encoded_iter().skip(key_offset).collect();
    //         let key_buf = encode_path(&key);

    //         let mut payload = Cursor::new(Vec::new());
    //         write_slice(&key_buf, &mut payload);

    //         // Encode value.
    //         // TODO: Improve performance by avoiding allocations.
    //         write_slice(value.as_ref(), &mut payload);

    //         write_list(&payload.into_inner(), &mut digest_buf);
    //         self.hash.0 = digest_buf.extract_or_finalize(&mut self.hash.1);
    //     }

    //     &self.hash.1[..self.hash.0]
    // }
}

// #[cfg(test)]
// mod test {
//     use super::*;
//     use sha3::Keccak256;
//     use slab::Slab;
//     use std::{iter::Copied, slice::Iter};

//     #[derive(Clone, Debug, Eq, PartialEq)]
//     struct MyNodePath(Vec<Nibble>);

//     impl TreePath for MyNodePath {
//         type Iterator<'a> = Copied<Iter<'a, Nibble>>;

//         fn encode(&self, mut target: impl std::io::Write) -> std::io::Result<()> {
//             let mut iter = self.0.iter().copied().peekable();
//             if self.0.len() % 2 == 1 {
//                 target.write_all(&[iter.next().unwrap() as u8])?;
//             }

//             while iter.peek().is_some() {
//                 let a = iter.next().unwrap() as u8;
//                 let b = iter.next().unwrap() as u8;

//                 target.write_all(&[(a << 4) | b])?;
//             }

//             Ok(())
//         }

//         fn encoded_iter(&self) -> Self::Iterator<'_> {
//             self.0.iter().copied()
//         }
//     }

//     #[derive(Clone, Copy, Debug, Eq, PartialEq)]
//     struct MyNodeValue([u8; 4]);

//     impl MyNodeValue {
//         pub fn new(value: i32) -> Self {
//             Self(value.to_be_bytes())
//         }
//     }

//     impl AsRef<[u8]> for MyNodeValue {
//         fn as_ref(&self) -> &[u8] {
//             &self.0
//         }
//     }

//     #[test]
//     fn new() {
//         let node = LeafNode::<MyNodePath, MyNodeValue, Keccak256>::new(0);

//         assert_eq!(node.value_ref, 0);
//     }

//     #[test]
//     fn get_some() {
//         let nodes = Slab::new();
//         let mut values = Slab::new();

//         let path = MyNodePath(vec![]);
//         let value = MyNodeValue::new(42);

//         let value_ref = values.insert((path.clone(), value));
//         let node = LeafNode::<_, _, Keccak256>::new(value_ref);

//         assert_eq!(
//             node.get(&nodes, &values, Offseted::new(path.encoded_iter())),
//             Some(&value),
//         );
//     }

//     #[test]
//     fn get_none() {
//         let nodes = Slab::new();
//         let mut values = Slab::new();

//         let path = MyNodePath(vec![Nibble::V0]);
//         let value = MyNodeValue::new(42);

//         let value_ref = values.insert((path, value));
//         let node = LeafNode::<_, _, Keccak256>::new(value_ref);

//         let path = MyNodePath(vec![Nibble::V1]);
//         assert_eq!(
//             node.get(&nodes, &values, Offseted::new(path.encoded_iter())),
//             None,
//         );
//     }

//     #[test]
//     #[should_panic]
//     fn get_iits() {
//         let nodes = Slab::new();
//         let values = Slab::new();

//         let path = MyNodePath(vec![Nibble::V0]);
//         let node = LeafNode::<MyNodePath, MyNodeValue, Keccak256>::new(0);

//         node.get(&nodes, &values, Offseted::new(path.encoded_iter()));
//     }
// }
