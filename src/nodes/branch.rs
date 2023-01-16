use super::LeafNode;
use crate::{
    nibble::NibbleSlice,
    node::{InsertAction, Node},
    util::{write_list, write_slice, DigestBuf, INVALID_REF},
    NodesStorage, ValuesStorage,
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
    choices: [Option<usize>; 16],
    value_ref: Option<usize>,

    hash: (usize, Output<H>),
    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> BranchNode<P, V, H>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
{
    pub fn new(choices: [Option<usize>; 16]) -> Self {
        Self {
            choices,
            value_ref: None,
            hash: (0, Default::default()),
            phantom: PhantomData,
        }
    }

    pub fn update_value_ref(&mut self, new_value_ref: Option<usize>) {
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
                        .get(child_ref)
                        .expect("inconsistent internal tree structure");

                    child_node.get(nodes, values, path)
                })
            })
            .or_else(|| {
                // Return internal value if present.
                self.value_ref.as_ref().map(|child_ref| {
                    let (_, value) = values
                        .get(*child_ref)
                        .expect("inconsistent internal tree structure");

                    value
                })
            })
    }

    pub fn insert(
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
                        .try_remove(*choice_ref)
                        .expect("inconsistent internal tree structure");

                    let (child_node, insert_action) = child_node.insert(nodes, values, path);
                    *choice_ref = nodes.insert(child_node);

                    insert_action.quantize_self(*choice_ref)
                }
                choice_ref => {
                    let child_ref = nodes.insert(LeafNode::new(INVALID_REF).into());
                    *choice_ref = Some(child_ref);

                    InsertAction::Insert(child_ref)
                }
            },
            None => self
                .value_ref
                .map(InsertAction::Insert)
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
                            .try_remove(*child_ref)
                            .expect("inconsistent internal tree structure");

                        payload.extend_from_slice(child_node.compute_hash(
                            nodes,
                            values,
                            key_offset + 1,
                        ));

                        *child_ref = nodes.insert(child_node);
                    }
                    None => payload.push(0x80),
                }
            }

            if let Some(value_ref) = self.value_ref {
                write_slice(
                    values
                        .get(value_ref)
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
    use crate::{nibble::Nibble, pmt_node, pmt_state};

    use super::*;
    use sha3::Keccak256;
    use slab::Slab;
    use std::{iter::Copied, slice::Iter};

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
    //         let node = BranchNode::<MyNodePath, MyNodeValue, Keccak256>::new({
    //             let mut choices = [None; 16];

    //             choices[2] = Some(2);
    //             choices[5] = Some(5);

    //             choices
    //         });

    //         assert_eq!(
    //             node.choices,
    //             [
    //                 None,
    //                 None,
    //                 Some(2),
    //                 None,
    //                 None,
    //                 Some(5),
    //                 None,
    //                 None,
    //                 None,
    //                 None,
    //                 None,
    //                 None,
    //                 None,
    //                 None,
    //                 None,
    //                 None,
    //             ],
    //         );
    //     }

    #[test]
    fn get_some() {
        let (mut nodes, mut values) = pmt_state!(Vec<u8>);

        let branch_node = pmt_node! { @(nodes, values)
            branch {
                0 => leaf { vec![0x00] => vec![0, 1, 2] },
                1 => leaf { vec![0x10] => vec![2, 1, 0] },
            }
        };

        // println!("{:x?}", &NibbleSlice::new(&[0]));

        assert_eq!(
            branch_node.get(&nodes, &values, NibbleSlice::new(&[0])),
            Some(&vec![0, 1, 2]),
        );
    }

    /*
    #[test]
    fn get_none() {
        let (nodes, values) = pmt_state!(Vec<u8>);


        let branch_node = pmt_node! { @(nodes, values)
            branch {
                0 => leaf { vec![1] => vec![0, 1, 2] },
                1 => leaf { vec![2] => vec![2, 1, 0] },
            }
        };


        let path = MyNodePath(vec![Nibble::V0]);
        let value = MyNodeValue::new(42);

        let value_ref = values.insert((path.clone(), value));
        let child_node = LeafNode::<MyNodePath, MyNodeValue, Keccak256>::new(value_ref);
        let child_ref = nodes.insert(child_node.into());

        let node = BranchNode::<_, _, Keccak256>::new({
            let mut choices = [None; 16];
            choices[path.encoded_iter().next().unwrap() as usize] = Some(child_ref);
            choices
        });

        let path = MyNodePath(vec![Nibble::V1]);
        assert_eq!(
            node.get(&nodes, &values, Offseted::new(path.encoded_iter())),
            None,
        );

    }
    */

    #[test]
    #[should_panic]
    fn get_inconsistent_internal_tree_structure() {
        let (nodes, values) = pmt_state!(Vec<u8>);

        let mut choices = [None; 16];
        choices[0] = Some(1234);

        let node = BranchNode::new(choices);

        node.get(&nodes, &values, NibbleSlice::new(&[0x0]));
    }
}
