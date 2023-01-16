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
        mut path: NibbleSlice,
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

            let mut path_branch = path.clone();
            path_branch.offset_add(offset);

            let (branch_node, mut insert_action) =
                if offset == 2 * path.as_ref().len() {
                    (
                        BranchNode::new({
                            let mut choices = [None; 16];
                            // TODO: Dedicated method.
                            choices[path_branch.next().unwrap() as usize] =
                                Some(nodes.insert(self.into()));
                            choices
                        }),
                        InsertAction::InsertSelf,
                    )
                } else if offset == 2 * value_path.as_ref().len() {
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
                            choices[path_branch.next().unwrap() as usize] = Some(child_ref);
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

#[cfg(test)]
mod test {
    use super::*;
    use crate::{pmt_node, pmt_state, util::Offseted};
    use sha3::Keccak256;

    #[test]
    fn new() {
        let node = LeafNode::<Vec<u8>, Vec<u8>, Keccak256>::new(0);
        assert_eq!(node.value_ref, 0);
    }

    #[test]
    fn get_some() {
        let (nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { vec![0x12] => vec![0x12, 0x34, 0x56, 0x78] }
        };

        assert_eq!(
            node.get(&nodes, &values, NibbleSlice::new(&[0x12]))
                .map(Vec::as_slice),
            Some([0x12, 0x34, 0x56, 0x78].as_slice()),
        );
    }

    #[test]
    fn get_none() {
        let (nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            leaf { vec![0x12] => vec![0x12, 0x34, 0x56, 0x78] }
        };

        assert_eq!(
            node.get(&nodes, &values, NibbleSlice::new(&[0x34]))
                .map(Vec::as_slice),
            None,
        );
    }

    #[test]
    #[should_panic]
    fn get_inconsistent_internal_tree_structure() {
        let (nodes, values) = pmt_state!(Vec<u8>);

        let path = NibbleSlice::new(&[0xFF]);
        let node = LeafNode::new(0);

        node.get(&nodes, &values, path);
    }
}
