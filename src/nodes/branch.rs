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
            match &mut self.choices[path_iter.next().unwrap() as usize] {
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

    pub fn remove<I>(
        mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &mut ValuesStorage<P, V, H>,
        mut path_iter: Offseted<I>,
    ) -> (Option<Node<P, V, H>>, Option<V>)
    where
        I: Iterator<Item = Nibble>,
    {
        let child_index = match path_iter.next() {
            Some(x) => x as usize,
            None => return (Some(self.into()), None),
        };

        let child_ref = match self.choices[child_index] {
            Some(x) => x,
            None => return (Some(self.into()), None),
        };

        let (new_node, old_value) = nodes
            .try_remove(child_ref)
            .expect("inconsistent internal tree structure")
            .remove(nodes, values, path_iter);

        let new_node = if let Some(new_node) = new_node {
            self.choices[child_index] = Some(nodes.insert(new_node));
            Some(self.into())
        } else {
            let choices = self
                .choices
                .iter()
                .copied()
                .try_fold(None, |acc, child_ref| match (acc, child_ref) {
                    (None, None) => Ok(None),
                    (None, Some(child_ref)) => Ok(Some(child_ref)),
                    (Some(acc), None) => Ok(Some(acc)),
                    (Some(_), Some(_)) => Err(()),
                })
                .ok();

            match choices {
                Some(x) => x.map(|child_ref| {
                    nodes
                        .try_remove(child_ref)
                        .expect("inconsistent internal tree structure")
                }),
                None => Some(self.into()),
            }
        };

        (new_node, old_value)
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util::build_value;
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

        let path = MyNodePath(vec![Nibble::V0]);
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

        let path = MyNodePath(vec![Nibble::V0]);
        let value = 42;

        let value_ref = values.insert(build_value::<_, _, Keccak256>(path.clone(), value));
        let child_node = LeafNode::<MyNodePath, i32, Keccak256>::new(value_ref);
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

    #[test]
    #[should_panic]
    fn get_iits() {
        let nodes = Slab::new();
        let values = Slab::new();

        let path = MyNodePath(vec![Nibble::V0]);
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
