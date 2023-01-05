use crate::nibble::Nibble;
use std::io::{self, Write};

pub trait TreePath {
    type Iterator<'a>: Iterator<Item = Nibble>
    where
        Self: 'a;

    fn encode(&self, target: impl Write) -> io::Result<()>;
    fn encoded_iter(&self) -> Self::Iterator<'_>;
}
