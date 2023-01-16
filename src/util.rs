use crate::Nibble;
use digest::{Digest, Output};
use std::io::{Cursor, Write};

pub const INVALID_REF: usize = usize::MAX;

pub fn write_slice(value: &[u8], mut target: impl Write) {
    if value.len() <= 55 {
        target.write_all(&[0x80 + value.len() as u8]).unwrap();
    } else {
        let len_bytes = value.len().to_be_bytes();
        let write_offset = len_bytes.iter().copied().take_while(|&x| x == 0).count();
        target
            .write_all(&[0xB7 + (len_bytes.len() - write_offset) as u8])
            .unwrap();
        target.write_all(&len_bytes[write_offset..]).unwrap();
    }

    target.write_all(value).unwrap();
}

pub fn write_list(payload: &[u8], mut target: impl Write) {
    if payload.len() <= 55 {
        target.write_all(&[0xC0 + payload.len() as u8]).unwrap();
    } else {
        let len_bytes = payload.len().to_be_bytes();
        let write_offset = len_bytes.iter().copied().take_while(|&x| x == 0).count();
        target
            .write_all(&[0xF7 + (len_bytes.len() - write_offset) as u8])
            .unwrap();
        target.write_all(&len_bytes[write_offset..]).unwrap();
    }

    target.write_all(payload).unwrap();
}

// TODO: Improve performance.
pub fn encode_path(nibbles: &[Nibble]) -> Vec<u8> {
    let flag = 0x20;
    if nibbles.len() % 2 == 1 {
        let flag = flag | 0x10;

        let mut target = Vec::new();
        target.push(flag | (nibbles[0] as u8));
        target.extend(
            nibbles[1..]
                .chunks(2)
                .map(|x| (u8::from(x[0]) << 4) | u8::from(x[1])),
        );

        target
    } else {
        Vec::from_iter(
            nibbles
                .chunks(2)
                .map(|x| (u8::from(x[0]) << 4) | u8::from(x[1])),
        )
    }
}

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
