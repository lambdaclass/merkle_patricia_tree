use crate::nibble::Nibble;
use std::io::{self, Write};

pub trait TreePath {
    type Iterator<'a>: Iterator<Item = Nibble>
    where
        Self: 'a;

    /// Encode the path for hashing.
    fn encode(&self, target: impl Write) -> io::Result<()>;

    /// Iterate over the encoded path.
    fn encoded_iter(&self) -> Self::Iterator<'_>;
}
