use super::LeafNode;
use crate::{
    hashing::{NodeHash, NodeHashRef, NodeHasher},
    nibble::NibbleSlice,
    node::{InsertAction, Node},
    NodeRef, NodesStorage, ValueRef, ValuesStorage,
};
use digest::Digest;
use std::marker::PhantomData;

#[derive(Clone, Debug)]
pub struct BranchNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    // The node zero is always the root, which cannot be a child.
    pub(crate) choices: [NodeRef; 16],
    pub(crate) value_ref: ValueRef,

    hash: NodeHash<H>,
    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> BranchNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    pub(crate) fn new(choices: [NodeRef; 16]) -> Self {
        Self {
            choices,
            value_ref: Default::default(),
            hash: Default::default(),
            phantom: PhantomData,
        }
    }

    pub(crate) fn update_value_ref(&mut self, new_value_ref: ValueRef) {
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
                let child_ref = self.choices[choice];
                if child_ref.is_valid() {
                    let child_node = nodes
                        .get(*child_ref)
                        .expect("inconsistent internal tree structure");

                    child_node.get(nodes, values, path)
                } else {
                    None
                }
            })
            .or_else(|| {
                // Return internal value if present.
                if self.value_ref.is_valid() {
                    let (_, value) = values
                        .get(*self.value_ref)
                        .expect("inconsistent internal tree structure");

                    Some(value)
                } else {
                    None
                }
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

        self.hash.mark_as_dirty();

        let insert_action = match path.next() {
            Some(choice) => match &mut self.choices[choice as usize] {
                choice_ref if !choice_ref.is_valid() => {
                    let child_ref = nodes.insert(LeafNode::new(Default::default()).into());
                    *choice_ref = NodeRef::new(child_ref);

                    InsertAction::Insert(NodeRef::new(child_ref))
                }
                choice_ref => {
                    let child_node = nodes
                        .try_remove(**choice_ref)
                        .expect("inconsistent internal tree structure");

                    let (child_node, insert_action) = child_node.insert(nodes, values, path);
                    *choice_ref = NodeRef::new(nodes.insert(child_node));

                    insert_action.quantize_self(*choice_ref)
                }
            },
            None => {
                if self.value_ref.is_valid() {
                    InsertAction::Replace(self.value_ref)
                } else {
                    InsertAction::InsertSelf
                }
            }
        };

        (self.into(), insert_action)
    }

    pub fn compute_hash(
        &self,
        nodes: &NodesStorage<P, V, H>,
        values: &ValuesStorage<P, V>,
        key_offset: usize,
    ) -> NodeHashRef<H> {
        self.hash.extract_ref().unwrap_or_else(|| {
            let mut children_len: usize = self
                .choices
                .iter()
                .map(|choice| {
                    choice
                        .is_valid()
                        .then(|| {
                            let child_node = nodes
                                .get(**choice)
                                .expect("inconsistent internal tree structure");

                            let child_hash_ref =
                                child_node.compute_hash(nodes, values, key_offset + 1);
                            match child_hash_ref {
                                NodeHashRef::Inline(x) => x.len(),
                                NodeHashRef::Hashed(x) => NodeHasher::<H>::bytes_len(x.len(), x[0]),
                            }
                        })
                        .unwrap_or(1)
                })
                .sum();

            if self.value_ref.is_valid() {
                let (_, value) = values
                    .get(*self.value_ref)
                    .expect("inconsistent internal tree structure");

                children_len += NodeHasher::<H>::bytes_len(
                    value.as_ref().len(),
                    value.as_ref().first().copied().unwrap_or_default(),
                );
            } else {
                children_len += 1;
            }

            let mut hasher = NodeHasher::new(&self.hash);
            hasher.write_list_header(children_len);

            self.choices.iter().for_each(|choice| {
                if choice.is_valid() {
                    let child_node = nodes
                        .get(**choice)
                        .expect("inconsistent internal tree structure");

                    let child_hash_ref = child_node.compute_hash(nodes, values, key_offset + 1);
                    match child_hash_ref {
                        NodeHashRef::Inline(x) => hasher.write_raw(&x),
                        NodeHashRef::Hashed(x) => hasher.write_bytes(&x),
                    }
                } else {
                    hasher.write_bytes(&[]);
                }
            });

            if self.value_ref.is_valid() {
                let (_, value) = values
                    .get(*self.value_ref)
                    .expect("inconsistent internal tree structure");

                hasher.write_bytes(value.as_ref());
            } else {
                hasher.write_bytes(&[]);
            }

            hasher.finalize()
        })
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
            let mut choices = [Default::default(); 16];

            choices[2] = NodeRef::new(2);
            choices[5] = NodeRef::new(5);

            choices
        });

        assert_eq!(
            node.choices,
            [
                Default::default(),
                Default::default(),
                NodeRef::new(2),
                Default::default(),
                Default::default(),
                NodeRef::new(5),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
                Default::default(),
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
        assert_eq!(insert_action, InsertAction::Insert(NodeRef::new(2)));
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

    #[test]
    fn compute_hash_two_choices() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            branch {
                2 => leaf { vec![0x20] => vec![0x20] },
                4 => leaf { vec![0x40] => vec![0x40] },
            }
        };

        assert_eq!(
            node.compute_hash(&nodes, &values, 0).as_ref(),
            &[
                0xD5, 0x80, 0x80, 0xC2, 0x30, 0x20, 0x80, 0xC2, 0x30, 0x40, 0x80, 0x80, 0x80, 0x80,
                0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
            ],
        );
    }

    #[test]
    fn compute_hash_all_choices() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            branch {
                0x0 => leaf { vec![0x00] => vec![0x00] },
                0x1 => leaf { vec![0x10] => vec![0x10] },
                0x2 => leaf { vec![0x20] => vec![0x20] },
                0x3 => leaf { vec![0x30] => vec![0x30] },
                0x4 => leaf { vec![0x40] => vec![0x40] },
                0x5 => leaf { vec![0x50] => vec![0x50] },
                0x6 => leaf { vec![0x60] => vec![0x60] },
                0x7 => leaf { vec![0x70] => vec![0x70] },
                0x8 => leaf { vec![0x80] => vec![0x80] },
                0x9 => leaf { vec![0x90] => vec![0x90] },
                0xA => leaf { vec![0xA0] => vec![0xA0] },
                0xB => leaf { vec![0xB0] => vec![0xB0] },
                0xC => leaf { vec![0xC0] => vec![0xC0] },
                0xD => leaf { vec![0xD0] => vec![0xD0] },
                0xE => leaf { vec![0xE0] => vec![0xE0] },
                0xF => leaf { vec![0xF0] => vec![0xF0] },
            }
        };

        assert_eq!(
            node.compute_hash(&nodes, &values, 0).as_ref(),
            &[
                0x0A, 0x3C, 0x06, 0x2D, 0x4A, 0xE3, 0x61, 0xEC, 0xC4, 0x82, 0x07, 0xB3, 0x2A, 0xDB,
                0x6A, 0x3A, 0x3F, 0x3E, 0x98, 0x33, 0xC8, 0x9C, 0x9A, 0x71, 0x66, 0x3F, 0x4E, 0xB5,
                0x61, 0x72, 0xD4, 0x9D,
            ],
        );
    }

    #[test]
    fn compute_hash_one_choice_with_value() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            branch {
                2 => leaf { vec![0x20] => vec![0x20] },
                4 => leaf { vec![0x40] => vec![0x40] },
            } with_leaf { vec![] => vec![] }
        };

        assert_eq!(
            node.compute_hash(&nodes, &values, 0).as_ref(),
            &[
                0xD5, 0x80, 0x80, 0xC2, 0x30, 0x20, 0x80, 0xC2, 0x30, 0x40, 0x80, 0x80, 0x80, 0x80,
                0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80, 0x80,
            ],
        );
    }

    #[test]
    fn compute_hash_all_choices_with_value() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let node = pmt_node! { @(nodes, values)
            branch {
                0x0 => leaf { vec![0x00] => vec![0x00] },
                0x1 => leaf { vec![0x10] => vec![0x10] },
                0x2 => leaf { vec![0x20] => vec![0x20] },
                0x3 => leaf { vec![0x30] => vec![0x30] },
                0x4 => leaf { vec![0x40] => vec![0x40] },
                0x5 => leaf { vec![0x50] => vec![0x50] },
                0x6 => leaf { vec![0x60] => vec![0x60] },
                0x7 => leaf { vec![0x70] => vec![0x70] },
                0x8 => leaf { vec![0x80] => vec![0x80] },
                0x9 => leaf { vec![0x90] => vec![0x90] },
                0xA => leaf { vec![0xA0] => vec![0xA0] },
                0xB => leaf { vec![0xB0] => vec![0xB0] },
                0xC => leaf { vec![0xC0] => vec![0xC0] },
                0xD => leaf { vec![0xD0] => vec![0xD0] },
                0xE => leaf { vec![0xE0] => vec![0xE0] },
                0xF => leaf { vec![0xF0] => vec![0xF0] },
            } with_leaf { vec![] => vec![] }
        };

        assert_eq!(
            node.compute_hash(&nodes, &values, 0).as_ref(),
            &[
                0x0A, 0x3C, 0x06, 0x2D, 0x4A, 0xE3, 0x61, 0xEC, 0xC4, 0x82, 0x07, 0xB3, 0x2A, 0xDB,
                0x6A, 0x3A, 0x3F, 0x3E, 0x98, 0x33, 0xC8, 0x9C, 0x9A, 0x71, 0x66, 0x3F, 0x4E, 0xB5,
                0x61, 0x72, 0xD4, 0x9D,
            ],
        );
    }
}
