use super::LeafNode;
use crate::{nibble::NibbleIterator, node::Node, util::build_value, TreePath};
use digest::{Digest, Output};
use slab::Slab;
use std::marker::PhantomData;

pub struct BranchNode<V, H>
where
    V: TreePath,
    H: Digest,
{
    // The node zero is always the root, which cannot be a child.
    choices: [Option<usize>; 16],

    // TODO: Hash?
    phantom: PhantomData<(V, H)>,
}

impl<V, H> BranchNode<V, H>
where
    V: TreePath,
    H: Digest,
{
    pub fn new(choices: [Option<usize>; 16]) -> Self {
        Self {
            choices,
            phantom: PhantomData,
        }
    }

    pub fn get<'a, I>(
        &self,
        nodes: &'a Slab<Node<V, H>>,
        values: &'a Slab<(Output<H>, V)>,
        full_path: &V::Path,
        mut path_iter: NibbleIterator<I>,
    ) -> Option<&'a V>
    where
        I: Iterator<Item = u8>,
    {
        // The nibble can be converted to a number, which corresponds to the choice index.
        match path_iter.next().map(u8::from).map(usize::from) {
            Some(nibble) => self.choices[nibble].and_then(|child_ref| {
                let child = nodes
                    .get(child_ref)
                    .expect("inconsistent internal tree structure");

                child.get(nodes, values, full_path, path_iter)
            }),
            None => None,
        }
    }

    pub fn insert<I>(
        mut self,
        nodes: &mut Slab<Node<V, H>>,
        values: &mut Slab<(Output<H>, V)>,
        full_path: &V::Path,
        mut path_iter: NibbleIterator<I>,
        value: V,
    ) -> (Node<V, H>, Option<V>)
    where
        I: Clone + Iterator<Item = u8>,
    {
        // If the path iterator is finished, convert the node into a leaf-branch. Otherwise insert
        // a new choice or delegate to a child if a choice is present.
        if path_iter.is_done() {
            // Convert to leaf-branch.
            let mut path_len = 0;
            let value_ref = values.insert(build_value::<V, H>(value, Some(&mut path_len)));

            ((self, LeafNode::new(path_len, value_ref)).into(), None)
        } else {
            match &mut self.choices[path_iter.next().unwrap() as u8 as usize] {
                Some(child_ref) => {
                    // Delegate to child.
                    let child = nodes
                        .try_remove(*child_ref)
                        .expect("inconsistent internal tree structure");

                    let (child, old_value) =
                        child.insert(nodes, values, full_path, path_iter, value);
                    *child_ref = nodes.insert(child);

                    (self.into(), old_value)
                }
                choice_ref => {
                    // Insert new choice.
                    let mut path_len = 0;
                    let value_ref = values.insert(build_value::<V, H>(value, Some(&mut path_len)));
                    *choice_ref = Some(nodes.insert(LeafNode::new(path_len, value_ref).into()));

                    (self.into(), None)
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use sha3::Keccak256;
    use std::io;

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct MyNode(String);

    impl TreePath for MyNode {
        type Path = String;

        fn path(&self) -> Self::Path {
            self.0.clone()
        }

        fn encode_path(path: &Self::Path, mut target: impl std::io::Write) -> io::Result<()> {
            target.write_all(path.as_bytes())
        }
    }

    #[test]
    fn new() {
        let node = BranchNode::<MyNode, Keccak256>::new({
            let mut choices = [None; 16];

            choices[2] = Some(2);
            choices[5] = Some(5);

            choices
        });

        assert_eq!(
            node.choices,
            [
                None,
                None,
                Some(2),
                None,
                None,
                Some(5),
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
        todo!()
    }

    #[test]
    fn get_none() {
        todo!()
    }

    #[test]
    #[should_panic]
    fn get_iits() {
        todo!()
    }
}
