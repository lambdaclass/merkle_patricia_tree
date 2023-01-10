use super::{BranchNode, LeafNode};
use crate::{
    nibble::Nibble,
    node::{InsertAction, Node},
    util::Offseted,
    NodesStorage, TreePath, ValuesStorage,
};
use digest::Digest;
use smallvec::SmallVec;
use std::marker::PhantomData;

#[derive(Clone, Debug)]
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
        values: &'a ValuesStorage<P, V>,
        mut path_iter: Offseted<I>,
    ) -> Option<&'a V>
    where
        I: Iterator<Item = Nibble>,
    {
        // Count the number of matching prefix nibbles (prefix).
        let prefix_match_len = path_iter.count_equals(&mut self.prefix.iter().copied().peekable());

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
        values: &mut ValuesStorage<P, V>,
        mut path_iter: Offseted<I>,
    ) -> (Node<P, V, H>, InsertAction)
    where
        I: Iterator<Item = Nibble>,
    {
        // Count the number of matching prefix nibbles (prefix) and check if the .
        let (prefix_match_len, prefix_fits) = {
            let prefix_match_len =
                path_iter.count_equals(&mut self.prefix.iter().copied().peekable());

            (prefix_match_len, path_iter.peek().is_none())
        };

        // If the entire prefix matches (matched len equals prefix len), call the child's insertion
        // logic.
        if prefix_match_len == self.prefix.len() {
            let child = nodes
                .try_remove(self.child_ref)
                .expect("inconsistent internal tree structure");

            let (child, insert_action) = child.insert(nodes, values, path_iter);
            self.child_ref = nodes.insert(child);

            let insert_action = insert_action.quantize_self(self.child_ref);
            (self.into(), insert_action)
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

                // The value will be inserted later on.
                // Note: The tree will be left inconsistent, but will be fixed later on.
                (
                    (
                        ExtensionNode::new(prefix, child_ref),
                        LeafNode::new(values.vacant_key()),
                    )
                        .into(),
                    InsertAction::Insert(child_ref),
                )
            } else if prefix_match_len == 0 {
                let mut choices = [None; 16];

                // Insert itself (after updating prefix).
                let index = self.prefix.remove(0) as usize;
                choices[index] = Some(nodes.insert(self.into()));

                // Create and insert new node.
                let index = path_iter.next().unwrap() as usize;
                let child_ref = nodes.insert(LeafNode::new(values.vacant_key()).into());
                choices[index] = Some(child_ref);

                (
                    BranchNode::new(choices).into(),
                    InsertAction::Insert(child_ref),
                )
            } else {
                // Extract the common prefix.
                let prefix: SmallVec<[Nibble; 16]> =
                    (&mut path_iter).take(prefix_match_len).collect();

                // Create and insert the branch node.
                let child_ref = {
                    let mut choices = [None; 16];

                    // Insert itself (after updating prefix).
                    let index = self.prefix[prefix_match_len] as usize;
                    self.prefix = self.prefix.into_iter().skip(prefix_match_len + 1).collect();
                    choices[index] = Some(nodes.insert(self.into()));

                    // Create and insert new node.
                    let index = path_iter.next().unwrap() as usize;
                    let child_ref = nodes.insert(LeafNode::new(values.vacant_key()).into());
                    choices[index] = Some(child_ref);

                    nodes.insert(BranchNode::new(choices).into())
                };

                (
                    ExtensionNode::new(prefix, child_ref).into(),
                    InsertAction::Insert(child_ref),
                )
            }
        }
    }

    pub fn remove<I>(
        mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        mut path_iter: Offseted<I>,
    ) -> (Option<Node<P, V, H>>, Option<V>)
    where
        I: Iterator<Item = Nibble>,
    {
        if self
            .prefix
            .iter()
            .copied()
            .eq((&mut path_iter).take(self.prefix.len()))
        {
            let (new_node, old_value) = nodes
                .try_remove(self.child_ref)
                .expect("inconsistent internal tree structure")
                .remove(nodes, values, path_iter);

            (
                new_node.map(|new_node| {
                    self.child_ref = nodes.insert(new_node);
                    self.into()
                }),
                old_value,
            )
        } else {
            (Some(self.into()), None)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use sha3::Keccak256;
    use slab::Slab;
    use std::{iter::Copied, slice::Iter};

    #[derive(Clone, Debug, Eq, PartialEq)]
    struct MyNodePath(Vec<Nibble>);

    impl TreePath for MyNodePath {
        type Iterator<'a> = Copied<Iter<'a, Nibble>>;

        fn encode(&self, mut target: impl std::io::Write) -> std::io::Result<()> {
            let mut iter = self.0.iter().copied().peekable();
            if self.0.len() % 2 == 1 {
                target.write_all(&[iter.next().unwrap() as u8])?;
            }

            while iter.peek().is_some() {
                let a = iter.next().unwrap() as u8;
                let b = iter.next().unwrap() as u8;

                target.write_all(&[(a << 4) | b])?;
            }

            Ok(())
        }

        fn encoded_iter(&self) -> Self::Iterator<'_> {
            self.0.iter().copied()
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

        let path = MyNodePath(vec![Nibble::V0]);
        let value = 42;

        let value_ref = values.insert((path.clone(), value));
        let child_node = LeafNode::<MyNodePath, i32, Keccak256>::new(value_ref);
        let child_ref = nodes.insert(child_node.into());

        let node = ExtensionNode::<_, _, Keccak256>::new(
            [path.encoded_iter().next().unwrap()].as_slice(),
            child_ref,
        );

        assert_eq!(
            node.get(&nodes, &values, Offseted::new(path.encoded_iter())),
            Some(&value),
        );
    }

    #[test]
    fn get_none() {
        let mut nodes = Slab::new();
        let mut values = Slab::new();

        let path = MyNodePath(vec![Nibble::V0]);
        let value = 42;

        let value_ref = values.insert((path.clone(), value));
        let child_node = LeafNode::<MyNodePath, i32, Keccak256>::new(value_ref);
        let child_ref = nodes.insert(child_node.into());

        let node = ExtensionNode::<_, _, Keccak256>::new(
            [path.encoded_iter().next().unwrap()].as_slice(),
            child_ref,
        );

        let path = MyNodePath(vec![Nibble::V1]);
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

        let path = MyNodePath(vec![Nibble::V0]);
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

    // Test for bug (.next() -> .peek(), l.78).
    #[test]
    fn test() {
        let mut nodes = Slab::new();
        let mut values = Slab::new();

        // let path = MyNodePath("\x00".to_string());
        let extension_node =
            ExtensionNode::<MyNodePath, (), Keccak256>::new([Nibble::V0].as_slice(), 0);

        let path = MyNodePath(vec![Nibble::V1]);
        // let leaf_node = LeafNode::new(0);

        println!(
            "{:#?}",
            extension_node.insert(&mut nodes, &mut values, Offseted::new(path.encoded_iter()))
        );
    }
}
