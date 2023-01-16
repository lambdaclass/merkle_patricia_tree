use super::LeafNode;
use crate::{
    nibble::NibbleSlice,
    node::{InsertAction, Node},
    util::{write_list, write_slice, DigestBuf},
    NodeRef, NodesStorage, ValueRef, ValuesStorage,
};
use digest::{Digest, Output};
use std::{io::Cursor, marker::PhantomData};

#[derive(Clone, Debug)]
pub struct BranchNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    // The node zero is always the root, which cannot be a child.
    choices: [Option<NodeRef>; 16],
    value_ref: Option<ValueRef>,

    hash: (usize, Output<H>),
    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> BranchNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    pub(crate) fn new(choices: [Option<NodeRef>; 16]) -> Self {
        Self {
            choices,
            value_ref: None,
            hash: (0, Default::default()),
            phantom: PhantomData,
        }
    }

    pub(crate) fn update_value_ref(&mut self, new_value_ref: Option<ValueRef>) {
        self.value_ref = new_value_ref;
    }

    pub fn get<'a>(
        &self,
        nodes: &'a NodesStorage<P, V, H>,
        values: &'a ValuesStorage<P, V>,
        mut path: NibbleSlice,
    ) -> Option<&'a V> {
        // If path is at the end, return to its own value if present.
        // Otherwise, check the corresponding choice and delegate accordingly if present.

        path.next()
            .map(usize::from)
            .and_then(|choice| {
                // Delegate to children if present.
                self.choices[choice].and_then(|child_ref| {
                    let child_node = nodes
                        .get(child_ref.0)
                        .expect("inconsistent internal tree structure");

                    child_node.get(nodes, values, path)
                })
            })
            .or_else(|| {
                // Return internal value if present.
                self.value_ref.as_ref().map(|child_ref| {
                    let (_, value) = values
                        .get(child_ref.0)
                        .expect("inconsistent internal tree structure");

                    value
                })
            })
    }

    pub(crate) fn insert(
        mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        mut path: NibbleSlice,
    ) -> (Node<P, V, H>, InsertAction) {
        // If path is at the end, insert or replace its own value.
        // Otherwise, check the corresponding choice and insert or delegate accordingly.

        self.hash.0 = 0;

        let insert_action = match path.next() {
            Some(choice) => match &mut self.choices[choice as usize] {
                Some(choice_ref) => {
                    let child_node = nodes
                        .try_remove(choice_ref.0)
                        .expect("inconsistent internal tree structure");

                    let (child_node, insert_action) = child_node.insert(nodes, values, path);
                    *choice_ref = NodeRef(nodes.insert(child_node));

                    insert_action.quantize_self(*choice_ref)
                }
                choice_ref => {
                    let child_ref = nodes.insert(LeafNode::new(Default::default()).into());
                    *choice_ref = Some(NodeRef(child_ref));

                    InsertAction::Insert(NodeRef(child_ref))
                }
            },
            None => self
                .value_ref
                .map(InsertAction::Replace)
                .unwrap_or(InsertAction::InsertSelf),
        };

        (self.into(), insert_action)
    }

    pub fn compute_hash(
        &mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &ValuesStorage<P, V>,
        key_offset: usize,
    ) -> &[u8] {
        if self.hash.0 == 0 {
            let mut digest_buf = DigestBuf::<H>::new();

            let mut payload = Vec::new();
            for choice in &mut self.choices {
                match choice {
                    Some(child_ref) => {
                        let mut child_node = nodes
                            .try_remove(child_ref.0)
                            .expect("inconsistent internal tree structure");

                        payload.extend_from_slice(child_node.compute_hash(
                            nodes,
                            values,
                            key_offset + 1,
                        ));

                        *child_ref = NodeRef(nodes.insert(child_node));
                    }
                    None => payload.push(0x80),
                }
            }

            if let Some(value_ref) = self.value_ref {
                write_slice(
                    values
                        .get(value_ref.0)
                        .expect("inconsistent internal tree structure")
                        .1
                        .as_ref(),
                    {
                        let mut cursor = Cursor::new(&mut payload);
                        cursor.set_position(cursor.get_ref().len() as u64);
                        cursor
                    },
                );
            }

            write_list(&payload, &mut digest_buf);
            self.hash.0 = digest_buf.extract_or_finalize(&mut self.hash.1);
        }

        &self.hash.1[..self.hash.0]
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{pmt_node, pmt_state};
    use sha3::Keccak256;

    #[test]
    fn new() {
        let node = BranchNode::<Vec<u8>, Vec<u8>, Keccak256>::new({
            let mut choices = [None; 16];

            choices[2] = Some(NodeRef(2));
            choices[5] = Some(NodeRef(5));

            choices
        });

        assert_eq!(
            node.choices,
            [
                None,
                None,
                Some(NodeRef(2)),
                None,
                None,
                Some(NodeRef(5)),
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
                None,
            ],
        );
    }

    #[test]
    fn get_some() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            branch {
                0 => leaf { vec![0x00] => vec![0x12, 0x34, 0x56, 0x78] },
                1 => leaf { vec![0x10] => vec![0x34, 0x56, 0x78, 0x9A] },
            }
        };

        assert_eq!(
            node.get(&nodes, &values, NibbleSlice::new(&[0x00]))
                .map(Vec::as_slice),
            Some([0x12, 0x34, 0x56, 0x78].as_slice()),
        );
        assert_eq!(
            node.get(&nodes, &values, NibbleSlice::new(&[0x10]))
                .map(Vec::as_slice),
            Some([0x34, 0x56, 0x78, 0x9A].as_slice()),
        );
    }

    #[test]
    fn get_none() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            branch {
                0 => leaf { vec![0x00] => vec![0x12, 0x34, 0x56, 0x78] },
                1 => leaf { vec![0x10] => vec![0x34, 0x56, 0x78, 0x9A] },
            }
        };

        assert_eq!(
            node.get(&nodes, &values, NibbleSlice::new(&[0x20]))
                .map(Vec::as_slice),
            None,
        );
    }

    #[test]
    fn insert_self() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            branch {
                0 => leaf { vec![0x00] => vec![0x12, 0x34, 0x56, 0x78] },
                1 => leaf { vec![0x10] => vec![0x34, 0x56, 0x78, 0x9A] },
            }
        };

        let (node, insert_action) = node.insert(&mut nodes, &mut values, NibbleSlice::new(&[]));
        let _ = match node {
            Node::Branch(x) => x,
            _ => panic!("expected a branch node"),
        };

        // TODO: Check node and children.
        assert_eq!(insert_action, InsertAction::InsertSelf);
    }

    #[test]
    fn insert_choice() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            branch {
                0 => leaf { vec![0x00] => vec![0x12, 0x34, 0x56, 0x78] },
                1 => leaf { vec![0x10] => vec![0x34, 0x56, 0x78, 0x9A] },
            }
        };

        let (node, insert_action) = node.insert(&mut nodes, &mut values, NibbleSlice::new(&[0x20]));
        let _ = match node {
            Node::Branch(x) => x,
            _ => panic!("expected a branch node"),
        };

        // TODO: Check node and children.
        assert_eq!(insert_action, InsertAction::Insert(NodeRef(2)));
    }

    #[test]
    fn insert_passthrough() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            branch {
                0 => leaf { vec![0x00] => vec![0x12, 0x34, 0x56, 0x78] },
                1 => leaf { vec![0x10] => vec![0x34, 0x56, 0x78, 0x9A] },
            }
        };

        // The extension node is ignored since it's irrelevant in this test.
        let (node, insert_action) = node.insert(&mut nodes, &mut values, {
            let mut nibble_slice = NibbleSlice::new(&[0x00]);
            nibble_slice.offset_add(2);
            nibble_slice
        });
        let _ = match node {
            Node::Branch(x) => x,
            _ => panic!("expected a branch node"),
        };

        // TODO: Check node and children.
        assert_eq!(insert_action, InsertAction::InsertSelf);
    }
}
