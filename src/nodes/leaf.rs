use crate::{
    nibble::{Nibble, NibbleIterator},
    node::Node,
    nodes::ExtensionNode,
    util::build_value,
    TreePath,
};
use digest::{Digest, Output};
use slab::Slab;
use smallvec::SmallVec;
use std::{marker::PhantomData, mem::replace};

pub struct LeafNode<V, H>
where
    V: TreePath,
    H: Digest,
{
    value_ref: usize,
    // hash: Option<<H::OutputSize as ArrayLength<u8>>::ArrayType>,

    // Path length, kept to reliably advance the iterator to after the current path.
    path_len: usize,

    phantom: PhantomData<(V, H)>,
}

impl<V, H> LeafNode<V, H>
where
    V: TreePath,
    H: Digest,
{
    pub fn new(path_len: usize, value_ref: usize) -> Self {
        Self {
            path_len,
            value_ref,
            // hash: None,
            phantom: PhantomData,
        }
    }

    pub fn get<'a, I>(
        &self,
        _nodes: &Slab<Node<V, H>>,
        values: &'a Slab<(Output<H>, V)>,
        full_path: &V::Path,
        _path_iter: NibbleIterator<I>,
    ) -> Option<&'a V>
    where
        I: Iterator<Item = u8>,
    {
        // Checking if full_path matches the value's path regardless of path_iter's contents since:
        //   - They will be empty if preceded by a branch at the last nibble.
        //   - They will not be empty in any other case.

        let (_, value) = values
            .get(self.value_ref)
            .expect("inconsistent internal tree structure");

        // TODO: Is this faster than hashing and comparing the path?
        //   Is this faster than adding a method to `TreePath` that compares paths?
        (full_path == &value.path()).then_some(value)
    }

    pub fn insert<I>(
        self,
        nodes: &mut Slab<Node<V, H>>,
        values: &mut Slab<(Output<H>, V)>,
        full_path: &V::Path,
        mut path_iter: NibbleIterator<I>,
        new_value: V,
    ) -> (Node<V, H>, Option<V>)
    where
        I: Clone + Iterator<Item = u8>,
    {
        // Checking if full_path matches the value's path regardless of path_iter's contents since:
        //   - They will be empty if preceded by a branch at the last nibble.
        //   - They will not be empty in any other case.

        let (_, value) = values
            .get_mut(self.value_ref)
            .expect("inconsistent internal tree structure");

        // TODO: Is this faster than hashing and comparing the path?
        //   Is this faster than adding a method to `TreePath` that compares paths?
        let value_path = value.path();
        if full_path == &value_path {
            // Replace and return old value.
            // The key shouldn't have changed in this case.
            (self.into(), Some(replace(value, new_value)))
        } else {
            // If the iterator is done (.position() fails), then it's a bug.
            let prefix_len = self.path_len - path_iter.position().unwrap() - 1;
            let prefix: SmallVec<[Nibble; 16]> = path_iter.take(prefix_len).collect();

            // Insert the child.
            let mut path_len = 0;
            let value_ref = values.insert(build_value::<V, H>(new_value, Some(&mut path_len)));
            let child_ref = nodes.insert(LeafNode::new(path_len, value_ref).into());

            ((ExtensionNode::new(prefix, child_ref), self).into(), None)
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
        let node = LeafNode::<MyNode, Keccak256>::new(12, 34);

        assert_eq!(node.path_len, 12);
        assert_eq!(node.value_ref, 34);
    }

    #[test]
    fn get_some() {
        let nodes = Slab::new();
        let mut values = Slab::<(Output<Keccak256>, _)>::new();

        let node_value = MyNode("hello world".to_string());
        let node_path = node_value.path();

        let mut path_len = 0;
        let value_ref = values.insert(build_value::<_, Keccak256>(node_value.clone(), Some(&mut path_len)));
        let node = LeafNode::<_, Keccak256>::new(0, value_ref);

        assert_eq!(
            node.get(
                &nodes,
                &values,
                &node_path,
                NibbleIterator::new(node_path.as_bytes().iter().copied())
            ),
            Some(&node_value),
        );
    }

    #[test]
    fn get_none() {
        let nodes = Slab::new();
        let mut values = Slab::<(Output<Keccak256>, _)>::new();

        let node_value = MyNode("hello world".to_string());

        let mut path_len = 0;
        let value_ref = values.insert(build_value::<_, Keccak256>(node_value.clone(), Some(&mut path_len)));
        let node = LeafNode::<_, Keccak256>::new(0, value_ref);

        let node_path = "invalid node".to_string();
        assert_eq!(
            node.get(
                &nodes,
                &values,
                &node_path,
                NibbleIterator::new(node_path.as_bytes().iter().copied())
            ),
            None,
        );
    }

    #[test]
    #[should_panic]
    fn get_iits() {
        let nodes = Slab::new();
        let values = Slab::<(Output<Keccak256>, _)>::new();

        let node_value = MyNode("hello world".to_string());
        let node_path = node_value.path();

        let node = LeafNode::<_, Keccak256>::new(0, 0);

        assert_eq!(
            node.get(
                &nodes,
                &values,
                &node_path,
                NibbleIterator::new(node_path.as_bytes().iter().copied())
            ),
            Some(&node_value),
        );
    }
}
