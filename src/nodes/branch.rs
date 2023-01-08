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

// #[cfg(test)]
// mod test {
//     use super::*;
//     use sha3::Keccak256;
//     use std::io;

//     #[derive(Clone, Debug, Eq, PartialEq)]
//     struct MyNode(String);

//     impl TreePath for MyNode {
//         type Path = String;

//         fn path(&self) -> Self::Path {
//             self.0.clone()
//         }

//         fn encode_path(path: &Self::Path, mut target: impl std::io::Write) -> io::Result<()> {
//             target.write_all(path.as_bytes())
//         }
//     }

//     #[test]
//     fn new() {
//         let node = BranchNode::<MyNode, Keccak256>::new({
//             let mut choices = [None; 16];

//             choices[2] = Some(2);
//             choices[5] = Some(5);

//             choices
//         });

//         assert_eq!(
//             node.choices,
//             [
//                 None,
//                 None,
//                 Some(2),
//                 None,
//                 None,
//                 Some(5),
//                 None,
//                 None,
//                 None,
//                 None,
//                 None,
//                 None,
//                 None,
//                 None,
//                 None,
//                 None,
//             ],
//         );
//     }

//     #[test]
//     fn get_some() {
//         todo!()
//     }

//     #[test]
//     fn get_none() {
//         todo!()
//     }

//     #[test]
//     #[should_panic]
//     fn get_iits() {
//         todo!()
//     }
// }
