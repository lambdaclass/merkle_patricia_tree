use crate::TreePath;
use digest::{Digest, Output};
use generic_array::ArrayLength;
use std::io::{Cursor, Write};

pub fn build_value<V, H>(
    value: V,
    target_len: Option<&mut usize>,
) -> (<H::OutputSize as ArrayLength<u8>>::ArrayType, V)
where
    V: TreePath,
    H: Digest,
    <H::OutputSize as ArrayLength<u8>>::ArrayType: From<Output<H>>,
{
    let mut digest_buf = DigestBuf::<H>::new();

    value.encode_self_path(&mut digest_buf).unwrap();
    let (hashed_path, path_len) = digest_buf.finalize();

    if let Some(target_len) = target_len {
        *target_len = path_len;
    }

    (hashed_path, value)
}

struct DigestBuf<H>
where
    H: Digest,
{
    hasher: H,
    buffer: Cursor<[u8; 256]>,
    len: usize,
}

impl<H> DigestBuf<H>
where
    H: Digest,
{
    pub fn new() -> Self {
        Self {
            hasher: H::new(),
            buffer: Cursor::new([0u8; 256]),
            len: 0,
        }
    }

    // TODO: To check: https://github.com/fizyk20/generic-array/issues/132
    pub fn finalize(mut self) -> (<H::OutputSize as ArrayLength<u8>>::ArrayType, usize) {
        // The .unwrap() next line is infallible (see flush implementation).
        self.flush().unwrap();
        (self.hasher.finalize().into(), self.len)
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
                self.len += self.buffer.get_ref().len();
            }
        }

        todo!()
    }

    fn flush(&mut self) -> std::io::Result<()> {
        let buffer = &self.buffer.get_ref()[..self.buffer.position() as usize];

        self.hasher.update(buffer);
        self.len += buffer.len();
        self.buffer.set_position(0);

        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use digest::OutputSizeUser;
    use sha3::Keccak256;
    use std::{any::type_name, io};

    struct MyNode(String);

    impl TreePath for MyNode {
        type Path = String;

        fn path(&self) -> Self::Path {
            self.0.clone()
        }

        fn encode_path(path: &Self::Path, mut target: impl std::io::Write) -> io::Result<()> {
            target.write_all(path.as_bytes())
        }
    }

    #[test]
    fn digest_buf_new() {
        DigestBuf::<Keccak256>::new();

        type A = <<Keccak256 as OutputSizeUser>::OutputSize as ArrayLength<u8>>::ArrayType;
        println!("{}", type_name::<A>());
    }
}
