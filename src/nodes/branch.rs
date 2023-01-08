use super::LeafNode;
use crate::{
    nibble::Nibble,
    node::{InsertAction, Node},
    util::Offseted,
    NodesStorage, TreePath, ValuesStorage,
};
use digest::Digest;
use std::marker::PhantomData;

#[derive(Clone, Debug)]
pub struct BranchNode<P, V, H>
where
    P: TreePath,
    H: Digest,
{
    // The node zero is always the root, which cannot be a child.
    choices: [Option<usize>; 16],

    // TODO: Hash?
    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> BranchNode<P, V, H>
where
    P: TreePath,
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
        nodes: &'a NodesStorage<P, V, H>,
        values: &'a ValuesStorage<P, V, H>,
        mut path_iter: Offseted<I>,
    ) -> Option<&'a V>
    where
        I: Iterator<Item = Nibble>,
    {
        // The nibble can be converted to a number, which corresponds to the choice index.
        match path_iter.next().map(u8::from).map(usize::from) {
            Some(nibble) => self.choices[nibble].and_then(|child_ref| {
                let child = nodes
                    .get(child_ref)
                    .expect("inconsistent internal tree structure");

                child.get(nodes, values, path_iter)
            }),
            None => None,
        }
    }

    pub fn insert<I>(
        mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V, H>,
        mut path_iter: Offseted<I>,
    ) -> (Node<P, V, H>, InsertAction)
    where
        I: Iterator<Item = Nibble>,
    {
        // If the path iterator is finished, convert the node into a leaf-branch. Otherwise insert
        // a new choice or delegate to a child if a choice is present.
        if path_iter.peek().is_none() {
            // Convert to leaf-branch (the tree will be left inconsistent, but will be fixed later
            // on).
            (
                (self, LeafNode::new(values.vacant_key())).into(),
                InsertAction::InsertSelf,
            )
        } else {
            match &mut self.choices[path_iter.next().unwrap() as u8 as usize] {
                Some(child_ref) => {
                    // Delegate to child.
                    let child = nodes
                        .try_remove(*child_ref)
                        .expect("inconsistent internal tree structure");

                    let (child, insert_action) = child.insert(nodes, values, path_iter);
                    *child_ref = nodes.insert(child);

                    let insert_action = insert_action.quantize_self(*child_ref);
                    (self.into(), insert_action)
                }
                choice_ref => {
                    // Insert new choice (the tree will be left inconsistent, but will be fixed
                    // later on).
                    let child_ref = nodes.insert(LeafNode::new(values.vacant_key()).into());
                    *choice_ref = Some(child_ref);

                    (self.into(), InsertAction::Insert(child_ref))
                }
            }
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::{util::build_value, NibbleIterator};
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
        let node = BranchNode::<MyNodePath, i32, Keccak256>::new({
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
        let mut nodes = Slab::new();
        let mut values = Slab::new();

        let path = MyNodePath("hello world".to_string());
        let value = 42;

        let value_ref = values.insert(build_value::<_, _, Keccak256>(path.clone(), value));
        let child_node = LeafNode::<MyNodePath, i32, Keccak256>::new(value_ref);
        let child_ref = nodes.insert(child_node.into());

        let node = BranchNode::<_, _, Keccak256>::new({
            let mut choices = [None; 16];
            choices[path.encoded_iter().next().unwrap() as usize] = Some(child_ref);
            choices
        });

        assert_eq!(
            node.get(&nodes, &values, Offseted::new(path.encoded_iter())),
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

        let node = BranchNode::<_, _, Keccak256>::new({
            let mut choices = [None; 16];
            choices[path.encoded_iter().next().unwrap() as usize] = Some(child_ref);
            choices
        });

        let path = MyNodePath("invalid node".to_string());
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

        let path = MyNodePath("hello world".to_string());
        let node = BranchNode::<MyNodePath, (), Keccak256>::new({
            let mut choices = [None; 16];
            choices[path.encoded_iter().next().unwrap() as usize] = Some(1234);
            choices
        });

        node.get(
            &nodes,
            &values,
            Offseted::new(path.encoded_iter().peekable()),
        );
    }
}
