use crate::nibble::{NibbleSlice, NibbleVec};
use digest::{Digest, Output};
use std::{cmp::min, mem::size_of};

#[derive(Clone, Debug, Eq, Hash, PartialEq)]
pub struct NodeHash<H>
where
    H: Digest,
{
    length: usize,
    hash_ref: Output<H>,
}

impl<H> NodeHash<H>
where
    H: Digest,
{
    pub fn mark_as_dirty(&mut self) {
        self.length = 0;
    }

    pub fn extract_ref(&self) -> Option<NodeHashRef<H>> {
        match self.length {
            0 => None,
            32 => Some(NodeHashRef::Hashed(&self.hash_ref)),
            l => Some(NodeHashRef::Inline(&self.hash_ref[..l])),
        }
    }
}

impl<H> Default for NodeHash<H>
where
    H: Digest,
{
    fn default() -> Self {
        Self {
            length: 0,
            hash_ref: Default::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum NodeHashRef<'a, H>
where
    H: Digest,
{
    Inline(&'a [u8]),
    Hashed(&'a Output<H>),
}

impl<'a, H> AsRef<[u8]> for NodeHashRef<'a, H>
where
    H: Digest,
{
    fn as_ref(&self) -> &[u8] {
        match *self {
            NodeHashRef::Inline(x) => x,
            NodeHashRef::Hashed(x) => x.as_slice(),
        }
    }
}

pub struct NodeHasher<'a, H>
where
    H: Digest,
{
    parent: &'a mut NodeHash<H>,
    hasher: Option<H>,
}

impl<'a, H> NodeHasher<'a, H>
where
    H: Digest,
{
    pub fn new(parent: &'a mut NodeHash<H>) -> Self {
        parent.length = 0;

        Self {
            parent,
            hasher: None,
        }
    }

    pub fn finalize(self) -> NodeHashRef<'a, H> {
        match self.hasher {
            Some(hasher) => {
                self.push_hash_update();
                self.parent.length = 32;
                NodeHashRef::Hashed(&self.parent.hash_ref)
            }
            None => NodeHashRef::Inline(&self.parent.hash_ref[..self.parent.length]),
        }
    }

    pub fn path_len_vec(value: &NibbleVec, kind: PathKind) -> usize {
        // TODO: Do not use iterators.
        Self::bytes_len((value.iter().count() >> 1) + 1, 0)
    }

    pub fn path_len_slice(value: &NibbleSlice, kind: PathKind) -> usize {
        // TODO: Do not use iterators.
        Self::bytes_len((value.clone().count() >> 1) + 1, 0)
    }

    pub fn bytes_len(value_len: usize, first_value: u8) -> usize {
        match value_len {
            1 if first_value < 128 => 1,
            l if l < 56 => l + 1,
            l => l + next_power_of_256(l) + 1,
        }
    }

    pub fn list_len(children_len: usize) -> usize {
        match children_len {
            l if l < 56 => l + 1,
            l => l + next_power_of_256(l) + 1,
        }
    }

    pub fn write_path_vec(&mut self, value: &NibbleVec, kind: PathKind) {
        let mut flag = kind.into_flag();

        // TODO: Do not use iterators.
        let nibble_count = value.iter().count();
        let nibble_iter = if nibble_count & 0x01 != 0 {
            let mut iter = value.iter();
            flag |= iter.next().unwrap() as u8;
            iter
        } else {
            value.iter()
        };

        let i2 = nibble_iter.clone().skip(1).step_by(2);
        for (a, b) in nibble_iter.step_by(2).zip(i2) {
            self.write_raw(&[(a as u8) << 4 | (b as u8)]);
        }
    }

    pub fn write_path_slice(&mut self, value: &NibbleSlice, kind: PathKind) {
        let mut flag = kind.into_flag();

        // TODO: Do not use iterators.
        let nibble_count = value.clone().count();
        let nibble_iter = if nibble_count & 0x01 != 0 {
            let mut iter = value.clone();
            flag |= iter.next().unwrap() as u8;
            iter
        } else {
            value.clone()
        };

        let i2 = nibble_iter.clone().skip(1).step_by(2);
        for (a, b) in nibble_iter.step_by(2).zip(i2) {
            self.write_raw(&[(a as u8) << 4 | (b as u8)]);
        }
    }

    pub fn write_bytes(&mut self, value: &[u8]) {
        match value.len() {
            1 if value[0] < 128 => {}
            l if l < 56 => self.write_raw(&[0x80u8 + l as u8]),
            l => {
                let l_len = next_power_of_256(l);
                self.write_raw(&[0xB7 + l_len as u8]);
                self.write_raw(&l.to_be_bytes()[size_of::<usize>() - l_len..]);
            }
        }

        self.write_raw(value);
    }

    pub fn write_list_header(&mut self, children_len: usize) {
        match children_len {
            l if l < 56 => self.write_raw(&[0xC0u8 + l as u8]),
            l => {
                let l_len = next_power_of_256(l);
                self.write_raw(&[0xF7 + l_len as u8]);
                self.write_raw(&l.to_be_bytes()[size_of::<usize>() - l_len..]);
            }
        }
    }

    fn write_raw(&mut self, value: &[u8]) {
        let current_pos = 0;
        while current_pos < value.len() {
            let copy_len = min(32 - self.parent.length, value.len() - current_pos);

            let target_slice =
                &mut self.parent.hash_ref[self.parent.length..self.parent.length + copy_len];
            let source_slice = &value[current_pos..current_pos + copy_len];
            target_slice.copy_from_slice(source_slice);

            current_pos += copy_len;
            self.parent.length += copy_len;

            if self.parent.length == 32 {
                self.push_hash_update();
            }
        }
    }

    fn push_hash_update(&mut self) {
        let hasher = self.hasher.get_or_insert_with(H::new);
        hasher.update(&self.parent.hash_ref[..self.parent.length]);

        self.parent.length = 0;
    }
}

#[derive(Clone, Copy, Debug, Eq, Hash, PartialEq)]
pub enum PathKind {
    Extension,
    Leaf,
}

impl PathKind {
    fn into_flag(self) -> u8 {
        match self {
            PathKind::Extension => 0x00,
            PathKind::Leaf => 0x20,
        }
    }
}

fn next_power_of_256(value: usize) -> usize {
    let bits_used = 8 * size_of::<usize>() - value.leading_zeros() as usize;
    bits_used.saturating_sub(1) >> 3
}

// use crate::Nibble;
// use digest::{Digest, Output};
// use std::{
//     io::{Cursor, Write},
//     iter::once,
// };

// pub fn write_slice(value: &[u8], mut target: impl Write) {
//     if value.len() <= 55 {
//         target.write_all(&[0x80 + value.len() as u8]).unwrap();
//     } else {
//         let len_bytes = value.len().to_be_bytes();
//         let write_offset = len_bytes.iter().copied().take_while(|&x| x == 0).count();
//         target
//             .write_all(&[0xB7 + (len_bytes.len() - write_offset) as u8])
//             .unwrap();
//         target.write_all(&len_bytes[write_offset..]).unwrap();
//     }

//     target.write_all(value).unwrap();
// }

// pub fn write_list(payload: &[u8], mut target: impl Write) {
//     if payload.len() <= 55 {
//         target.write_all(&[0xC0 + payload.len() as u8]).unwrap();
//     } else {
//         let len_bytes = payload.len().to_be_bytes();
//         let write_offset = len_bytes.iter().copied().take_while(|&x| x == 0).count();
//         target
//             .write_all(&[0xF7 + (len_bytes.len() - write_offset) as u8])
//             .unwrap();
//         target.write_all(&len_bytes[write_offset..]).unwrap();
//     }

//     target.write_all(payload).unwrap();
// }

// // TODO: Improve performance.
// pub fn encode_path(nibbles: &[Nibble]) -> Vec<u8> {
//     let flag = 0x20;
//     if nibbles.len() % 2 != 0 {
//         let flag = flag | 0x10;

//         let mut target = Vec::new();
//         target.push(flag | (nibbles[0] as u8));
//         target.extend(
//             nibbles[1..]
//                 .chunks(2)
//                 .map(|x| (u8::from(x[0]) << 4) | u8::from(x[1])),
//         );

//         target
//     } else {
//         Vec::from_iter(
//             once(flag).chain(
//                 nibbles
//                     .chunks(2)
//                     .map(|x| (u8::from(x[0]) << 4) | u8::from(x[1])),
//             ),
//         )
//     }
// }

// pub struct DigestBuf<H>
// where
//     H: Digest,
// {
//     hasher: H,
//     buffer: Cursor<[u8; 256]>,
//     updated: bool,
// }

// impl<H> DigestBuf<H>
// where
//     H: Digest,
// {
//     pub fn new() -> Self {
//         Self {
//             hasher: H::new(),
//             buffer: Cursor::new([0u8; 256]),
//             updated: false,
//         }
//     }

//     pub fn extract_or_finalize(mut self, target: &mut Output<H>) -> usize {
//         if self.updated || self.buffer.position() >= 32 {
//             self.flush_update();
//             self.hasher.finalize_into(target);
//             32
//         } else {
//             let pos = self.buffer.position() as usize;
//             target[..pos].copy_from_slice(&self.buffer.get_ref()[..pos]);
//             pos
//         }
//     }

//     pub fn finalize(mut self) -> Output<H> {
//         self.flush_update();
//         self.hasher.finalize()
//     }

//     fn flush_update(&mut self) {
//         let buffer = &self.buffer.get_ref()[..self.buffer.position() as usize];

//         self.hasher.update(buffer);
//         self.buffer.set_position(0);
//         self.updated = true;
//     }
// }

// impl<H> Write for DigestBuf<H>
// where
//     H: Digest,
// {
//     fn write(&mut self, buf: &[u8]) -> std::io::Result<usize> {
//         println!("buf = {buf:02X?}");
//         let mut pos = 0;
//         while pos != buf.len() {
//             pos += self.buffer.write(&buf[pos..])?;
//             if self.buffer.position() as usize == self.buffer.get_ref().len() {
//                 self.hasher.update(self.buffer.get_ref());
//                 self.buffer.set_position(0);
//                 self.updated = true;
//             }
//         }

//         Ok(pos)
//     }

//     fn flush(&mut self) -> std::io::Result<()> {
//         Ok(())
//     }
// }
