use super::ExtensionNode;
use crate::{
    nibble::Nibble,
    node::Node,
    util::{build_value, Offseted},
    NodesStorage, TreePath, ValuesStorage,
};
use digest::Digest;
use smallvec::SmallVec;
use std::{iter::Peekable, marker::PhantomData, mem::replace};

pub struct LeafNode<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    value_ref: usize,
    // hash: Option<<H::OutputSize as ArrayLength<u8>>::ArrayType>,
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
            // hash: None,
            phantom: PhantomData,
        }
    }

    pub fn get<'a, I>(
        &self,
        _nodes: &NodesStorage<P, V, H>,
        values: &'a ValuesStorage<P, V, H>,
        path_iter: Offseted<Peekable<I>>,
    ) -> Option<&'a V>
    where
        I: Iterator<Item = Nibble>,
    {
        // Retrieve the value storage to compare paths and return the value if there's a match.
        let (value_path, _, value) = values
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
        values: &mut ValuesStorage<P, V, H>,
        mut path_iter: Offseted<Peekable<I>>,
        new_path: P,
        new_value: V,
    ) -> (Node<P, V, H>, Option<V>)
    where
        I: Iterator<Item = Nibble>,
    {
        // Retrieve the value storage to compare paths and overwrite the value if there's a match.
        let (value_path, _, value) = values
            .get_mut(self.value_ref)
            .expect("inconsistent internal tree structure");

        // Count the number of matching prefix nibbles (prefix) and check if the paths are equal.
        let path_offset = path_iter.offset();
        let (prefix_match_len, prefix_eq) = {
            let mut other_iter = value_path.encoded_iter().skip(path_offset);

            // Count number of equal nibbles. Paths are completely equal if both iterators have
            // finished.
            (
                (&mut path_iter)
                    .zip(&mut other_iter)
                    .take_while(|(a, b)| *a == *b)
                    .count(),
                path_iter.next().is_none() && other_iter.next().is_none(),
            )
        };

        if prefix_eq {
            // Replace and return old value.
            // The key shouldn't have changed in this case.
            (self.into(), Some(replace(value, new_value)))
        } else {
            // Insert an extension node.
            let prefix: SmallVec<[Nibble; 16]> = value_path
                .encoded_iter()
                .skip(path_offset)
                .take(prefix_match_len)
                .collect();

            // Insert the child.
            let value_ref = values.insert(build_value::<P, V, H>(new_path, new_value));
            let child_ref = nodes.insert(LeafNode::new(value_ref).into());

            ((ExtensionNode::new(prefix, child_ref), self).into(), None)
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
        let node = LeafNode::<MyNodePath, i32, Keccak256>::new(42);

        assert_eq!(node.value_ref, 42);
    }

    #[test]
    fn get_some() {
        let nodes = Slab::new();
        let mut values = Slab::new();

        let path = MyNodePath("hello world".to_string());
        let value = 42;

        let value_ref = values.insert(build_value::<_, _, Keccak256>(path.clone(), value));
        let node = LeafNode::<_, _, Keccak256>::new(value_ref);

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
        let nodes = Slab::new();
        let mut values = Slab::new();

        let path = MyNodePath("hello world".to_string());
        let value = 42;

        let value_ref = values.insert(build_value::<_, _, Keccak256>(path, value));
        let node = LeafNode::<_, _, Keccak256>::new(value_ref);

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
        let node = LeafNode::<MyNodePath, (), Keccak256>::new(0);

        node.get(
            &nodes,
            &values,
            Offseted::new(path.encoded_iter().peekable()),
        );
    }
}
