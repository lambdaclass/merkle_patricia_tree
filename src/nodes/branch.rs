use super::LeafNode;
use crate::{
    nibble::Nibble,
    node::{InsertAction, Node},
    util::{write_list, write_slice, DigestBuf, Offseted},
    NodesStorage, TreePath, ValuesStorage,
};
use digest::{Digest, Output};
use std::{io::Cursor, marker::PhantomData};

#[derive(Clone, Debug)]
pub struct BranchNode<P, V, H>
where
    P: TreePath,
    V: AsRef<[u8]>,
    H: Digest,
{
    // The node zero is always the root, which cannot be a child.
    choices: [Option<usize>; 16],
    value_ref: Option<usize>,

    hash: (usize, Output<H>),
    phantom: PhantomData<(P, V, H)>,
}

impl<P, V, H> BranchNode<P, V, H>
where
    P: TreePath,
    V: AsRef<[u8]>,
    H: Digest,
{
    pub fn new(choices: [Option<usize>; 16]) -> Self {
        Self {
            choices,
            value_ref: None,
            hash: (0, Default::default()),
            phantom: PhantomData,
        }
    }

    pub fn update_value_ref(&mut self, new_value_ref: Option<usize>) {
        self.value_ref = new_value_ref;
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
        values: &mut ValuesStorage<P, V>,
        mut path_iter: Offseted<I>,
    ) -> (Node<P, V, H>, InsertAction)
    where
        I: Iterator<Item = Nibble>,
    {
        // If the path iterator is finished, insert or update the contained value. Otherwise insert
        // a new choice or delegate to a child if a choice is present.
        if path_iter.peek().is_none() {
            let insert_action = self
                .value_ref
                .map(InsertAction::Insert)
                .unwrap_or(InsertAction::InsertSelf);

            (self.into(), insert_action)
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
                    self.hash.0 = 0; // Mark hash as dirty.
                    (self.into(), insert_action)
                }
                choice_ref => {
                    // Insert new choice (the tree will be left inconsistent, but will be fixed
                    // later on).
                    let child_ref = nodes.insert(LeafNode::new(values.vacant_key()).into());
                    *choice_ref = Some(child_ref);

                    self.hash.0 = 0; // Mark hash as dirty.
                    (self.into(), InsertAction::Insert(child_ref))
                }
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

        if old_value.is_some() {
            self.hash.0 = 0; // Mark hash as dirty.
        }

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
                Some(x) if self.value_ref.is_none() => x.map(|child_ref| {
                    nodes
                        .try_remove(child_ref)
                        .expect("inconsistent internal tree structure")
                }),
                _ => Some(self.into()),
            }
        };

        (new_node, old_value)
    }

    pub fn compute_hash(
        &mut self,
        nodes: &mut NodesStorage<P, V, H>,
        values: &ValuesStorage<P, V>,
        key_offset: usize,
    ) -> &[u8] {
        if self.hash.0 == 0 {
            let mut digest_buf = DigestBuf::<H>::new();

            let mut payload = Vec::new();
            for choice in &mut self.choices {
                match choice {
                    Some(child_ref) => {
                        let mut child_node = nodes
                            .try_remove(*child_ref)
                            .expect("inconsistent internal tree structure");

                        payload.extend_from_slice(child_node.compute_hash(
                            nodes,
                            values,
                            key_offset + 1,
                        ));

                        *child_ref = nodes.insert(child_node);
                    }
                    None => payload.push(0x80),
                }
            }

            if let Some(value_ref) = self.value_ref {
                write_slice(
                    values
                        .get(value_ref)
                        .expect("inconsistent internal tree structure")
                        .1
                        .as_ref(),
                    {
                        let mut cursor = Cursor::new(&mut payload);
                        cursor.set_position(cursor.get_ref().len() as u64);
                        cursor
                    },
                );
            }

            write_list(&payload, &mut digest_buf);
            self.hash.0 = digest_buf.extract_or_finalize(&mut self.hash.1);
        }

        &self.hash.1[..self.hash.0]
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

    #[derive(Clone, Copy, Debug, Eq, PartialEq)]
    struct MyNodeValue([u8; 4]);

    impl MyNodeValue {
        pub fn new(value: i32) -> Self {
            Self(value.to_be_bytes())
        }
    }

    impl AsRef<[u8]> for MyNodeValue {
        fn as_ref(&self) -> &[u8] {
            &self.0
        }
    }

    #[test]
    fn new() {
        let node = BranchNode::<MyNodePath, MyNodeValue, Keccak256>::new({
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
        let value = MyNodeValue::new(42);

        let value_ref = values.insert((path.clone(), value));
        let child_node = LeafNode::<MyNodePath, MyNodeValue, Keccak256>::new(value_ref);
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
        let value = MyNodeValue::new(42);

        let value_ref = values.insert((path.clone(), value));
        let child_node = LeafNode::<MyNodePath, MyNodeValue, Keccak256>::new(value_ref);
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
        let node = BranchNode::<MyNodePath, MyNodeValue, Keccak256>::new({
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
