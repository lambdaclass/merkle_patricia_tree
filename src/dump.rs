use crate::{
    node::Node,
    nodes::{BranchNode, ExtensionNode, LeafNode},
    Encode, NodeRef, PatriciaMerkleTree,
};
use digest::Digest;
use std::io::Write;

pub struct TreeDump<'a, P, V, H, W>
where
    P: Encode,
    V: Encode,
    H: Digest,
    W: Write,
{
    parent: &'a PatriciaMerkleTree<P, V, H>,
    writer: W,

    indent: usize,
}

impl<'a, P, V, H, W> TreeDump<'a, P, V, H, W>
where
    P: Encode,
    V: Encode,
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

        writeln!(self.writer).unwrap();
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
            let (path, value) = self
                .parent
                .values
                .get(*branch_node.value_ref)
                .expect("inconsistent internal tree structure");

            let path = path.encode();
            let value = value.encode();
            write!(
                self.writer,
                "{indent}}} with_value {{ {path:02x?} => {value:02x?} }}"
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
        let (path, value) = self
            .parent
            .values
            .get(*leaf_node.value_ref)
            .expect("inconsistent internal tree structure");

        let path = path.encode();
        let value = value.encode();
        write!(self.writer, "leaf {{ {path:02x?} => {value:02x?} }}").unwrap();
    }
}
