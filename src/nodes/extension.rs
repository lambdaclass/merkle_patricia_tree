use super::{BranchNode, LeafNode};
use crate::{
    nibble::Nibble,
    node::Node,
    util::{build_value, Offseted},
    NodesStorage, TreePath, ValuesStorage,
};
use digest::Digest;
use smallvec::SmallVec;
use std::{iter::Peekable, marker::PhantomData};

pub struct ExtensionNode<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    prefix: SmallVec<[Nibble; 16]>,
    // The child node may be a branch or an extension (when there's a node between extensions).
    child_ref: usize,

    // TODO: Hash?
    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> ExtensionNode<P, V, H>
where
    P: TreePath,
    H: Digest,
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
        nodes: &'a NodesStorage<P, V, H>,
        values: &'a ValuesStorage<P, V, H>,
        mut path_iter: Offseted<Peekable<I>>,
    ) -> Option<&'a V>
    where
        I: Iterator<Item = Nibble>,
    {
        // Count the number of matching prefix nibbles (prefix).
        let prefix_match_len = (&mut path_iter)
            .zip(self.prefix.iter().copied())
            .take_while(|(a, b)| a == b)
            .count();

        // If the entire prefix matches (matched len equals prefix len), call the child's get logic.
        // Otherwise, there's no matching node.
        if prefix_match_len == self.prefix.len() {
            let child = nodes
                .get(self.child_ref)
                .expect("inconsistent internal tree structure");

            child.get(nodes, values, path_iter)
        } else {
            None
        }
    }

    pub fn insert<I>(
        mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V, H>,
        mut path_iter: Offseted<Peekable<I>>,
        path: P,
        value: V,
    ) -> (Node<P, V, H>, Option<V>)
    where
        I: Iterator<Item = Nibble>,
    {
        // Count the number of matching prefix nibbles (prefix) and check if the .
        let (prefix_match_len, prefix_fits) = {
            let prefix_match_len = (&mut path_iter)
                .zip(self.prefix.iter().copied())
                .take_while(|(a, b)| a == b)
                .count();

            (prefix_match_len, path_iter.next().is_none())
        };

        // If the entire prefix matches (matched len equals prefix len), call the child's insertion
        // logic.
        if prefix_match_len == self.prefix.len() {
            let child = nodes
                .try_remove(self.child_ref)
                .expect("inconsistent internal tree structure");

            let (child, old_value) = child.insert(nodes, values, path_iter, path, value);
            self.child_ref = nodes.insert(child);

            (self.into(), old_value)
        } else {
            // If the new value's path is contained within the prefix, split the prefix with a
            // leaf-extension node, followed by an extension. Otherwise, insert a branch node or
            // an extension followed by a branch as needed.
            if prefix_fits {
                // Collect the new prefix.
                let prefix: SmallVec<[Nibble; 16]> = path_iter.collect();

                // Update self's prefix and insert itself (will be a child now).
                self.prefix = self.prefix.into_iter().skip(prefix.len()).collect();
                let child_ref = nodes.insert(self.into());

                // Insert the value for the new node.
                let value_ref = values.insert(build_value::<P, V, H>(path, value));

                (
                    (
                        ExtensionNode::new(prefix, child_ref),
                        LeafNode::new(value_ref),
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
                let value_ref = values.insert(build_value::<P, V, H>(path, value));
                choices[index] = Some(nodes.insert(LeafNode::new(value_ref).into()));

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
                    let value_ref = values.insert(build_value::<P, V, H>(path, value));
                    choices[index] = Some(nodes.insert(LeafNode::new(value_ref).into()));

                    nodes.insert(BranchNode::new(choices).into())
                };

                (ExtensionNode::new(prefix, child_ref).into(), None)
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::nibble::NibbleIterator;
    use sha3::Keccak256;
    use slab::Slab;
    use std::{io, str::Bytes};

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct MyNodePath(String);

    impl TreePath for MyNodePath {
        type Iterator<'a> = NibbleIterator<Bytes<'a>>;

        fn encode(&self, mut target: impl io::Write) -> io::Result<()> {
            target.write_all(self.0.as_bytes())
        }

        fn encoded_iter(&self) -> Self::Iterator<'_> {
            NibbleIterator::new(self.0.bytes())
        }
    }

    #[test]
    fn new() {
        let node = ExtensionNode::<MyNodePath, i32, Keccak256>::new(
            [Nibble::V0, Nibble::V1, Nibble::V2].as_slice(),
            12,
        );

        assert_eq!(
            node.prefix.as_slice(),
            [Nibble::V0, Nibble::V1, Nibble::V2].as_slice(),
        );
        assert_eq!(node.child_ref, 12);
    }

    #[test]
    fn get_some() {
        let mut nodes = Slab::new();
        let mut values = Slab::new();

        let path = MyNodePath("hello world".to_string());
        let value = 42;

        let value_ref = values.insert(build_value::<_, _, Keccak256>(path.clone(), value));
        let child_node = LeafNode::<MyNodePath, i32, Keccak256>::new(value_ref);
        let child_ref = nodes.insert(child_node.into());

        let node = ExtensionNode::<_, _, Keccak256>::new(
            [path.encoded_iter().next().unwrap()].as_slice(),
            child_ref,
        );

        assert_eq!(
            node.get(
                &nodes,
                &values,
                Offseted::new(path.encoded_iter().peekable()),
            ),
            Some(&value),
        );
    }

    #[test]
    fn get_none() {
        let mut nodes = Slab::new();
        let mut values = Slab::new();

        let path = MyNodePath("hello world".to_string());
        let value = 42;

        let value_ref = values.insert(build_value::<_, _, Keccak256>(path.clone(), value));
        let child_node = LeafNode::<MyNodePath, i32, Keccak256>::new(value_ref);
        let child_ref = nodes.insert(child_node.into());

        let node = ExtensionNode::<_, _, Keccak256>::new(
            [path.encoded_iter().next().unwrap()].as_slice(),
            child_ref,
        );

        let path = MyNodePath("invalid node".to_string());
        assert_eq!(
            node.get(
                &nodes,
                &values,
                Offseted::new(path.encoded_iter().peekable()),
            ),
            None,
        );
    }

    #[test]
    #[should_panic]
    fn get_iits() {
        let nodes = Slab::new();
        let values = Slab::new();

        let path = MyNodePath("hello world".to_string());
        let node = ExtensionNode::<MyNodePath, (), Keccak256>::new(
            [path.encoded_iter().next().unwrap()].as_slice(),
            1234,
        );

        node.get(
            &nodes,
            &values,
            Offseted::new(path.encoded_iter().peekable()),
        );
    }
}
