use super::BranchNode;
use crate::{
    nibble::{NibbleSlice, NibbleVec},
    node::{InsertAction, Node},
    NodesStorage, ValuesStorage,
};
use digest::{Digest, Output};
use std::marker::PhantomData;

#[derive(Clone, Debug)]
pub struct ExtensionNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    prefix: NibbleVec,
    // The child node may only be a branch, but it's not included directly by value to avoid
    // inflating `Node`'s size too much.
    child_ref: usize,

    hash: (usize, Output<H>),
    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> ExtensionNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    pub fn new(prefix: NibbleVec, child_ref: usize) -> Self {
        Self {
            prefix,
            child_ref,
            hash: (0, Default::default()),
            phantom: PhantomData,
        }
    }

    pub fn get<'a>(
        &self,
        nodes: &'a NodesStorage<P, V, H>,
        values: &'a ValuesStorage<P, V>,
        mut path: NibbleSlice,
    ) -> Option<&'a V> {
        // If the path is prefixed by this node's prefix, delegate to its child.
        // Otherwise, no value is present.

        path.skip_prefix(&self.prefix)
            .then(|| {
                let child_node = nodes
                    .get(self.child_ref)
                    .expect("inconsistent internal tree structure");

                child_node.get(nodes, values, path)
            })
            .flatten()
    }

    pub fn insert(
        mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        mut path: NibbleSlice,
    ) -> (Node<P, V, H>, InsertAction) {
        // [x] extension { [0], child } -> branch { 0 => child } with_value !
        // [x] extension { [0], child } -> extension { [0], child }

        // [ ] extension { [0, 1], child } -> branch { 0 => extension { [1], child } } with_value !
        // [ ] extension { [0, 1], child } -> extension { [0], branch { 1 => child } with_value ! }
        // [x] extension { [0, 1], child } -> extension { [0, 1], child }

        // [ ] extension { [0, 1, 2], child } -> branch { 0 => extension { [1, 2], child } } with_value !
        // [ ] extension { [0, 1, 2], child } -> extension { [0], branch { 1 => extension { [2], child } } with_value ! }
        // [ ] extension { [0, 1, 2], child } -> extension { [0, 1], branch { 2 => child } with_value ! }
        // [x] extension { [0, 1, 2], child } -> extension { [0, 1, 2], child }

        self.hash.0 = 0;

        if path.skip_prefix(&self.prefix) {
            let child_node = nodes
                .try_remove(self.child_ref)
                .expect("inconsistent internal tree structure");

            let (child_node, insert_action) = child_node.insert(nodes, values, path);
            self.child_ref = nodes.insert(child_node);

            let insert_action = insert_action.quantize_self(self.child_ref);
            (self.into(), insert_action)
        } else {
            // TODO: Implement dedicated method (avoid half-byte iterators).
            let offset = self
                .prefix
                .iter()
                .zip(path.clone())
                .take_while(|(a, b)| a == b)
                .count();
            assert!(offset < self.prefix.iter().count(), "{:#02x?}, {:#02x?}", self.prefix, path);
            let (left_prefix, choice, right_prefix) = self.prefix.split_extract_at(offset);

            let left_prefix = (!left_prefix.is_empty()).then_some(left_prefix);
            let right_prefix = (!right_prefix.is_empty()).then_some(right_prefix);

            // TODO: Prefix right node (if any, child is self.child_ref).
            let right_prefix_node = right_prefix
                .map(|right_prefix| {
                    nodes.insert(ExtensionNode::new(right_prefix, self.child_ref).into())
                })
                .unwrap_or(self.child_ref);

            // TODO: Branch node (child is prefix right or self.child_ref).
            let branch_node = BranchNode::new({
                let mut choices = [None; 16];
                choices[choice as usize] = Some(right_prefix_node);
                choices
            })
            .into();

            // TODO: Prefix left node (if any, child is branch_node).
            match left_prefix {
                Some(left_prefix) => {
                    let branch_ref = nodes.insert(branch_node);

                    (
                        ExtensionNode::new(left_prefix, branch_ref).into(),
                        InsertAction::Insert(branch_ref),
                    )
                }
                None => (branch_node, InsertAction::InsertSelf),
            }
        }
    }

    // pub fn remove<I>(
    //     mut self,
    //     nodes: &mut NodesStorage<P, V, H>,
    //     values: &mut ValuesStorage<P, V>,
    //     mut path_iter: Offseted<I>,
    // ) -> (Option<Node<P, V, H>>, Option<V>)
    // where
    //     I: Iterator<Item = Nibble>,
    // {
    //     if self
    //         .prefix
    //         .iter()
    //         .copied()
    //         .eq((&mut path_iter).take(self.prefix.len()))
    //     {
    //         let (new_node, old_value) = nodes
    //             .try_remove(self.child_ref)
    //             .expect("inconsistent internal tree structure")
    //             .remove(nodes, values, path_iter);

    //         if old_value.is_some() {
    //             self.hash.0 = 0; // Mark hash as dirty.
    //         }

    //         (
    //             new_node.map(|new_node| {
    //                 self.child_ref = nodes.insert(new_node);
    //                 self.into()
    //             }),
    //             old_value,
    //         )
    //     } else {
    //         (Some(self.into()), None)
    //     }
    // }

    // pub fn compute_hash(
    //     &mut self,
    //     nodes: &mut NodesStorage<P, V, H>,
    //     values: &ValuesStorage<P, V>,
    //     key_offset: usize,
    // ) -> &[u8] {
    //     if self.hash.0 == 0 {
    //         let mut payload = Cursor::new(Vec::new());

    //         let mut digest_buf = DigestBuf::<H>::new();

    //         let prefix = encode_path(&self.prefix);
    //         write_slice(&prefix, &mut payload);

    //         let mut child = nodes
    //             .try_remove(self.child_ref)
    //             .expect("inconsistent internal tree structure");
    //         let child_hash = child.compute_hash(nodes, values, key_offset + self.prefix.len());
    //         write_slice(child_hash, &mut payload);

    //         write_list(&payload.into_inner(), &mut digest_buf);
    //         self.hash.0 = digest_buf.extract_or_finalize(&mut self.hash.1);
    //     }

    //     &self.hash.1
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
//         let node = ExtensionNode::<MyNodePath, MyNodeValue, Keccak256>::new(
//             [Nibble::V0, Nibble::V1, Nibble::V2].as_slice(),
//             12,
//         );

//         assert_eq!(
//             node.prefix.as_slice(),
//             [Nibble::V0, Nibble::V1, Nibble::V2].as_slice(),
//         );
//         assert_eq!(node.child_ref, 12);
//     }

//     #[test]
//     fn get_some() {
//         let mut nodes = Slab::new();
//         let mut values = Slab::new();

//         let path = MyNodePath(vec![Nibble::V0]);
//         let value = MyNodeValue::new(42);

//         let value_ref = values.insert((path.clone(), value));
//         let child_node = LeafNode::<MyNodePath, MyNodeValue, Keccak256>::new(value_ref);
//         let child_ref = nodes.insert(child_node.into());

//         let node = ExtensionNode::<_, _, Keccak256>::new(
//             [path.encoded_iter().next().unwrap()].as_slice(),
//             child_ref,
//         );

//         assert_eq!(
//             node.get(&nodes, &values, Offseted::new(path.encoded_iter())),
//             Some(&value),
//         );
//     }

//     #[test]
//     fn get_none() {
//         let mut nodes = Slab::new();
//         let mut values = Slab::new();

//         let path = MyNodePath(vec![Nibble::V0]);
//         let value = MyNodeValue::new(42);

//         let value_ref = values.insert((path.clone(), value));
//         let child_node = LeafNode::<MyNodePath, MyNodeValue, Keccak256>::new(value_ref);
//         let child_ref = nodes.insert(child_node.into());

//         let node = ExtensionNode::<_, _, Keccak256>::new(
//             [path.encoded_iter().next().unwrap()].as_slice(),
//             child_ref,
//         );

//         let path = MyNodePath(vec![Nibble::V1]);
//         assert_eq!(
//             node.get(
//                 &nodes,
//                 &values,
//                 Offseted::new(path.encoded_iter().peekable()),
//             ),
//             None,
//         );
//     }

//     #[test]
//     #[should_panic]
//     fn get_iits() {
//         let nodes = Slab::new();
//         let values = Slab::new();

//         let path = MyNodePath(vec![Nibble::V0]);
//         let node = ExtensionNode::<MyNodePath, MyNodeValue, Keccak256>::new(
//             [path.encoded_iter().next().unwrap()].as_slice(),
//             1234,
//         );

//         node.get(
//             &nodes,
//             &values,
//             Offseted::new(path.encoded_iter().peekable()),
//         );
//     }

//     // Test for bug (.next() -> .peek(), l.78).
//     #[test]
//     fn test() {
//         let mut nodes = Slab::new();
//         let mut values = Slab::new();

//         // let path = MyNodePath("\x00".to_string());
//         let extension_node =
//             ExtensionNode::<MyNodePath, MyNodeValue, Keccak256>::new([Nibble::V0].as_slice(), 0);

//         let path = MyNodePath(vec![Nibble::V1]);
//         // let leaf_node = LeafNode::new(0);

//         println!(
//             "{:#?}",
//             extension_node.insert(&mut nodes, &mut values, Offseted::new(path.encoded_iter()))
//         );
//     }
// }
