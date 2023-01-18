//! Temporary (for now). Just for testing.

#![allow(unused)]

use crate::{
    node::Node,
    nodes::{BranchNode, ExtensionNode, LeafNode},
    NodeRef, PatriciaMerkleTree, ValueRef,
};
use digest::Digest;
use std::{io::Write, iter::repeat};

pub struct TreeDump<'a, P, V, H, W>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
    W: Write,
{
    parent: &'a PatriciaMerkleTree<P, V, H>,
    writer: W,

    indent: usize,
}

impl<'a, P, V, H, W> TreeDump<'a, P, V, H, W>
where
    P: AsRef<[u8]>,
    V: AsRef<[u8]>,
    H: Digest,
    W: Write,
{
    pub fn new(parent: &'a PatriciaMerkleTree<P, V, H>, writer: W, indent: usize) -> Self {
        Self {
            parent,
            writer,
            indent,
        }
    }

    pub fn dump(mut self) {
        let indent = " ".repeat(self.indent);
        write!(self.writer, "{indent}").unwrap();

        if !self.parent.root_ref.is_valid() {
            writeln!(self.writer, "(nil)").unwrap()
        } else {
            self.write_node(self.parent.root_ref);
            writeln!(self.writer).unwrap();
        }
    }

    fn write_node(&mut self, node_ref: NodeRef) {
        let node = self
            .parent
            .nodes
            .get(*node_ref)
            .expect("inconsistent internal tree structure");

        match node {
            Node::Branch(branch_node) => self.write_branch(branch_node),
            Node::Extension(extension_node) => self.write_extension(extension_node),
            Node::Leaf(leaf_node) => self.write_leaf(leaf_node),
        }
    }

    fn write_branch(&mut self, branch_node: &BranchNode<P, V, H>) {
        writeln!(self.writer, "branch {{").unwrap();
        self.indent += 4;
        let indent = " ".repeat(self.indent);
        for (index, choice) in branch_node.choices.iter().enumerate() {
            if !choice.is_valid() {
                continue;
            }

            write!(self.writer, "{indent}{index:01x} => ").unwrap();
            self.write_node(*choice);
            writeln!(self.writer, ",").unwrap();
        }
        self.indent -= 4;

        let indent = " ".repeat(self.indent);
        if !branch_node.value_ref.is_valid() {
            write!(self.writer, "{indent}}}").unwrap();
        } else {
            let (key, value) = self
                .parent
                .values
                .get(*branch_node.value_ref)
                .expect("inconsistent internal tree structure");

            let key = key.as_ref();
            let value = value.as_ref();
            write!(
                self.writer,
                "{indent}}} with_value {{ {key:02x?} => {value:02x?} }}"
            )
            .unwrap();
        }
    }

    fn write_extension(&mut self, extension_node: &ExtensionNode<P, V, H>) {
        let prefix = extension_node
            .prefix
            .iter()
            .map(|x| match x as u8 {
                x if (0..10).contains(&x) => (b'0' + x) as char,
                x => (b'A' + (x - 10)) as char,
            })
            .collect::<String>();

        write!(self.writer, "extension {{ {prefix}, ").unwrap();
        self.write_node(extension_node.child_ref);
        write!(self.writer, " }}").unwrap();
    }

    fn write_leaf(&mut self, leaf_node: &LeafNode<P, V, H>) {
        let (key, value) = self
            .parent
            .values
            .get(*leaf_node.value_ref)
            .expect("inconsistent internal tree structure");

        let key = key.as_ref();
        let value = value.as_ref();
        write!(self.writer, "leaf {{ {key:02x?} => {value:02x?} }}").unwrap();
    }
}
