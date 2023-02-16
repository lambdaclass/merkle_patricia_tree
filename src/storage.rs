use crate::node::Node;
use slab::Slab;
use std::ops::Deref;

const INVALID_REF: usize = usize::MAX;

pub type NodesStorage<P, V, H> = Slab<Node<P, V, H>>;
pub type ValuesStorage<P, V> = Slab<(P, V)>;

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct NodeRef(usize);

impl NodeRef {
    pub fn new(value: usize) -> Self {
        assert_ne!(value, INVALID_REF);
        Self(value)
    }

    pub const fn is_valid(&self) -> bool {
        self.0 != INVALID_REF
    }
}

impl Default for NodeRef {
    fn default() -> Self {
        Self(INVALID_REF)
    }
}

impl Deref for NodeRef {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
#[repr(transparent)]
pub struct ValueRef(usize);

impl ValueRef {
    pub fn new(value: usize) -> Self {
        assert_ne!(value, INVALID_REF);
        Self(value)
    }

    pub const fn is_valid(&self) -> bool {
        self.0 != INVALID_REF
    }
}

impl Default for ValueRef {
    fn default() -> Self {
        Self(INVALID_REF)
    }
}

impl Deref for ValueRef {
    type Target = usize;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}
