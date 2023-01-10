use super::ExtensionNode;
use crate::{
    nibble::Nibble,
    node::{InsertAction, Node},
    util::{DigestBuf, Offseted},
    NodesStorage, TreePath, ValuesStorage,
};
use digest::{Digest, Output};
use smallvec::SmallVec;
use std::{io::Write, marker::PhantomData};

#[derive(Clone, Debug)]
pub struct LeafNode<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    value_ref: usize,
    hash: (usize, Output<H>),

    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> LeafNode<P, V, H>
where
    P: TreePath,
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

    pub fn get<'a, I>(
        &self,
        _nodes: &NodesStorage<P, V, H>,
        values: &'a ValuesStorage<P, V>,
        path_iter: Offseted<I>,
    ) -> Option<&'a V>
    where
        I: Iterator<Item = Nibble>,
    {
        // Retrieve the value storage to compare paths and return the value if there's a match.
        let (value_path, value) = values
            .get(self.value_ref)
            .expect("inconsistent internal tree structure");

        // Compare remaining paths since everything before that should already be equal.
        let path_offset = path_iter.offset();
        path_iter
            .eq(value_path.encoded_iter().skip(path_offset))
            .then_some(value)
    }

    pub fn insert<I>(
        self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        mut path_iter: Offseted<I>,
    ) -> (Node<P, V, H>, InsertAction)
    where
        I: Iterator<Item = Nibble>,
    {
        // Retrieve the value storage to compare paths and overwrite the value if there's a match.
        let (value_path, _) = values
            .get_mut(self.value_ref)
            .expect("inconsistentinternal tree structure");

        // Count the number of matching prefix nibbles (prefix) and check if the paths are equal.
        let path_offset = path_iter.offset();
        let (prefix_match_len, prefix_eq) = {
            let mut other_iter = value_path.encoded_iter().skip(path_offset).peekable();

            // Count number of equal nibbles. Paths are completely equal if both iterators have
            // finished.
            (
                path_iter.count_equals(&mut other_iter),
                path_iter.next().is_none() && other_iter.next().is_none(),
            )
        };

        if prefix_eq {
            // Replace and return old value.
            // The key shouldn't have changed in this case.
            let insert_action = InsertAction::Replace(self.value_ref);
            (self.into(), insert_action)
        } else {
            // Insert an extension node.
            let prefix: SmallVec<[Nibble; 16]> = value_path
                .encoded_iter()
                .skip(path_offset)
                .take(prefix_match_len)
                .collect();

            // Insert the child (the tree will be left inconsistent, but will be fixed later on).
            let child_ref = nodes.insert(LeafNode::new(values.vacant_key()).into());

            (
                (ExtensionNode::new(prefix, child_ref), self).into(),
                InsertAction::Insert(child_ref),
            )
        }
    }

    pub fn remove<I>(
        self,
        _nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V>,
        path_iter: Offseted<I>,
    ) -> (Option<Node<P, V, H>>, Option<V>)
    where
        I: Iterator<Item = Nibble>,
    {
        // Retrieve the value storage to compare paths and return the value if there's a match.
        let (value_path, _) = values
            .get(self.value_ref)
            .expect("inconsistent internal tree structure");

        // Compare remaining paths since everything before that should already be equal, then return
        // the value if they match.
        let path_offset = path_iter.offset();
        path_iter
            .eq(value_path.encoded_iter().skip(path_offset))
            .then(|| (None, Some(values.remove(self.value_ref).1)))
            .unwrap_or((Some(self.into()), None))
    }

    // pub fn compute_hash(&mut self, values: &ValuesStorage<P, V>, key_offset: usize) -> &[u8] {
    //     if self.hash.0 == 0 {
    //         let (key, _, value) = values
    //             .get(self.value_ref)
    //             .expect("inconsistent internal tree structure");

    //         let mut digest_buf = DigestBuf::<H>::new();

    //         // Encode list prefix (length = 2).
    //         digest_buf.write_all(&[0xC2]).unwrap();

    //         // TODO: Encode key.
    //         // TODO: Improve performance by avoiding allocations.
    //         let key: Vec<_> = key.encoded_iter().skip(key_offset).collect();
    //         if key.len() % 2 == 1 {
    //             key.insert(0, 0x20)
    //         }

    //         // TODO: Encode value.

    //         self.hash.0 = digest_buf.extract_or_finalize(&mut self.hash.1);
    //     }

    //     &self.hash.1[..self.hash.0]
    // }
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
        let node = LeafNode::<MyNodePath, i32, Keccak256>::new(42);

        assert_eq!(node.value_ref, 42);
    }

    #[test]
    fn get_some() {
        let nodes = Slab::new();
        let mut values = Slab::new();

        let path = MyNodePath(vec![]);
        let value = 42;

        let value_ref = values.insert((path.clone(), value));
        let node = LeafNode::<_, _, Keccak256>::new(value_ref);

        assert_eq!(
            node.get(&nodes, &values, Offseted::new(path.encoded_iter())),
            Some(&value),
        );
    }

    #[test]
    fn get_none() {
        let nodes = Slab::new();
        let mut values = Slab::new();

        let path = MyNodePath(vec![Nibble::V0]);
        let value = 42;

        let value_ref = values.insert((path, value));
        let node = LeafNode::<_, _, Keccak256>::new(value_ref);

        let path = MyNodePath(vec![Nibble::V1]);
        assert_eq!(
            node.get(&nodes, &values, Offseted::new(path.encoded_iter())),
            None,
        );
    }

    #[test]
    #[should_panic]
    fn get_iits() {
        let nodes = Slab::new();
        let values = Slab::new();

        let path = MyNodePath(vec![Nibble::V0]);
        let node = LeafNode::<MyNodePath, (), Keccak256>::new(0);

        node.get(&nodes, &values, Offseted::new(path.encoded_iter()));
    }
}
