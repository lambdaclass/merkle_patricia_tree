use super::{BranchNode, LeafNode};
use crate::{
    nibble::{Nibble, NibbleIterator},
    node::Node,
    util::build_value,
    TreePath,
};
use digest::{Digest, Output};
use generic_array::ArrayLength;
use slab::Slab;
use smallvec::SmallVec;
use std::marker::PhantomData;

pub struct ExtensionNode<V, H>
where
    V: TreePath,
    H: Digest,
{
    prefix: SmallVec<[Nibble; 16]>,
    // The child node may be a branch or an extension (when there's a node between extensions).
    child_ref: usize,

    // TODO: Hash?
    phantom: PhantomData<(V, H)>,
}

impl<V, H> ExtensionNode<V, H>
where
    V: TreePath,
    H: Digest,
    <H::OutputSize as ArrayLength<u8>>::ArrayType: std::convert::From<Output<H>>,
{
    pub fn new(prefix: impl Into<SmallVec<[Nibble; 16]>>, child_ref: usize) -> Self {
        Self {
            prefix: prefix.into(),
            child_ref,
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
        let prefix_match_len = (&mut path_iter)
            .zip(self.prefix.iter().copied())
            .take_while(|(a, b)| a == b)
            .count();

        // If the entire prefix matches (matched len equals prefix len), call the child's get logic.
        if prefix_match_len == self.prefix.len() {
            let child = nodes
                .get(self.child_ref)
                .expect("inconsistent internal tree structure");

            child.get(nodes, values, full_path, path_iter)
        } else {
            None
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
        let prefix_match_len = path_iter
            .clone()
            .zip(self.prefix.iter().copied())
            .take_while(|(a, b)| a == b)
            .count();

        // If the entire prefix matches (matched len equals prefix len), call the child's insertion
        // logic.
        if prefix_match_len == self.prefix.len() {
            let child = nodes
                .try_remove(self.child_ref)
                .expect("inconsistent internal tree structure");

            let (child, old_value) = child.insert(nodes, values, full_path, path_iter, value);
            self.child_ref = nodes.insert(child);

            (self.into(), old_value)
        } else {
            // If the new value's path is contained within the prefix, split the prefix with a
            // leaf-extension node, followed by an extension. Otherwise, insert a branch node or
            // an extension followed by a branch as needed.
            if path_iter.is_done() {
                // Collect the new prefix.
                // Insert itself since it'll be a child now.
                let prefix: SmallVec<[Nibble; 16]> = path_iter.collect();
                let child_ref = nodes.insert(self.into());

                // Insert the value for the new node.
                let mut path_len = 0;
                let value_ref = values.insert(build_value::<V, H>(value, Some(&mut path_len)));

                (
                    (
                        ExtensionNode::new(prefix, child_ref),
                        LeafNode::new(path_len, value_ref),
                    )
                        .into(),
                    None,
                )
            } else if prefix_match_len == 0 {
                let mut choices = [None; 16];

                // Insert itself (after updating prefix).
                let index = self.prefix.remove(0) as u8 as usize;
                choices[index] = Some(nodes.insert(self.into()));

                // Create and insert new node.
                let index = path_iter.next().unwrap() as u8 as usize;
                let mut path_len = 0;
                let value_ref = values.insert(build_value::<V, H>(value, Some(&mut path_len)));
                choices[index] = Some(nodes.insert(LeafNode::new(path_len, value_ref).into()));

                (BranchNode::new(choices).into(), None)
            } else {
                // Extract the common prefix.
                let prefix: SmallVec<[Nibble; 16]> =
                    (&mut path_iter).take(prefix_match_len).collect();

                // Create and insert the branch node.
                let child_ref = {
                    let mut choices = [None; 16];

                    // Insert itself (after updating prefix).
                    let index = self.prefix[prefix_match_len] as u8 as usize;
                    self.prefix = self.prefix.into_iter().skip(prefix_match_len + 1).collect();
                    choices[index] = Some(nodes.insert(self.into()));

                    // Create and insert new node.
                    let index = path_iter.next().unwrap() as u8 as usize;
                    let mut path_len = 0;
                    let value_ref = values.insert(build_value::<V, H>(value, Some(&mut path_len)));
                    choices[index] = Some(nodes.insert(LeafNode::new(path_len, value_ref).into()));

                    nodes.insert(BranchNode::new(choices).into())
                };

                (ExtensionNode::new(prefix, child_ref).into(), None)
            }
        }
    }
}
