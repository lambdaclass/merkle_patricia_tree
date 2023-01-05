use crate::TreePath;
use digest::{Digest, Output};
use std::{
    io::{Cursor, Write},
    ops::{Deref, DerefMut},
};

pub fn build_value<P, V, H>(path: P, value: V) -> (P, Output<H>, V)
where
    P: TreePath,
    H: Digest,
{
    let mut digest_buf = DigestBuf::<H>::new();

    // `DigestBuf` should never return an error.
    path.encode(&mut digest_buf).unwrap();
    let hashed_path = digest_buf.finalize();

    (path, hashed_path, value)
}

struct DigestBuf<H>
where
    H: Digest,
{
    hasher: H,
    buffer: Cursor<[u8; 256]>,
}

impl<H> DigestBuf<H>
where
    H: Digest,
{
    pub fn new() -> Self {
        Self {
            hasher: H::new(),
            buffer: Cursor::new([0u8; 256]),
        }
    }

    // TODO: To check: https://github.com/fizyk20/generic-array/issues/132
    pub fn finalize(mut self) -> Output<H> {
        // The .unwrap() next line is infallible (see flush implementation).
        self.flush().unwrap();
        self.hasher.finalize()
    }
}

impl<H> Write for DigestBuf<H>
where
    H: Digest,
{
    fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
        let mut pos = 0;
        while pos != buf.len() {
            pos += self.buffer.write(&buf[pos..])?;
            if self.buffer.position() as usize == self.buffer.get_ref().len() {
                self.hasher.update(self.buffer.get_ref());
                self.buffer.set_position(0);
            }
        }

        Ok(pos)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let buffer = &self.buffer.get_ref()[..self.buffer.position() as usize];

        self.hasher.update(buffer);
        self.buffer.set_position(0);

        Ok(())
    }
}

pub struct Offseted<I>(I, usize)
where
    I: Iterator;

impl<I> Offseted<I>
where
    I: Iterator,
{
    pub fn new(inner: I) -> Self {
        Self(inner, 0)
    }

    pub fn offset(&self) -> usize {
        self.1
    }
}

impl<I> Iterator for Offseted<I>
where
    I: Iterator,
{
    type Item = I::Item;

    fn next(&mut self) -> Option<Self::Item> {
        self.0.next().map(|x| {
            self.1 += 1;
            x
        })
    }
}

impl<I> Deref for Offseted<I>
where
    I: Iterator,
{
    type Target = I;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<I> DerefMut for Offseted<I>
where
    I: Iterator,
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
