use super::LeafNode;
use crate::{nibble::NibbleIterator, node::Node, util::build_value, TreePath};
use digest::{Digest, Output};
use generic_array::ArrayLength;
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
    <H::OutputSize as ArrayLength<u8>>::ArrayType: std::convert::From<Output<H>>,
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
        values: &'a Slab<(<H::OutputSize as ArrayLength<u8>>::ArrayType, V)>,
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
        values: &mut Slab<(<H::OutputSize as ArrayLength<u8>>::ArrayType, V)>,
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
