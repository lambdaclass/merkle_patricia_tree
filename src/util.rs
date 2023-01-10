use digest::{Digest, Output};
use std::{
    io::{Cursor, Write},
    iter::Peekable,
};

pub struct DigestBuf<H>
where
    H: Digest,
{
    hasher: H,
    buffer: Cursor<[u8; 256]>,
    updated: bool,
}

impl<H> DigestBuf<H>
where
    H: Digest,
{
    pub fn new() -> Self {
        Self {
            hasher: H::new(),
            buffer: Cursor::new([0u8; 256]),
            updated: false,
        }
    }

    pub fn extract_or_finalize(mut self, target: &mut Output<H>) -> usize {
        if self.updated || self.buffer.position() >= 32 {
            self.flush_update();
            self.hasher.finalize_into(target);
            32
        } else {
            let pos = self.buffer.position() as usize;
            target[..pos].copy_from_slice(&self.buffer.get_ref()[..pos]);
            pos
        }
    }

    pub fn finalize(mut self) -> Output<H> {
        self.flush_update();
        self.hasher.finalize()
    }

    fn flush_update(&mut self) {
        let buffer = &self.buffer.get_ref()[..self.buffer.position() as usize];

        self.hasher.update(buffer);
        self.buffer.set_position(0);
        self.updated = true;
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
                self.updated = true;
            }
        }

        Ok(pos)
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

pub struct Offseted<I>(Peekable<I>, usize)
where
    I: Iterator;

impl<I> Offseted<I>
where
    I: Iterator,
{
    pub fn new(inner: I) -> Self {
        Self(inner.peekable(), 0)
    }

    pub fn offset(&self) -> usize {
        self.1
    }

    pub fn peek(&mut self) -> Option<&I::Item> {
        self.0.peek()
    }

    pub fn count_equals<I2>(&mut self, rhs: &mut Peekable<I2>) -> usize
    where
        I2: Iterator,
        I2::Item: PartialEq<I::Item>,
    {
        let mut count = 0;
        while self.0.next_if(|x| rhs.next_if_eq(x).is_some()).is_some() {
            count += 1;
        }
        self.1 += count;
        count
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
