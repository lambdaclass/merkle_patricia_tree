use std::iter::Peekable;

#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum Nibble {
    V0 = 0,
    V1 = 1,
    V2 = 2,
    V3 = 3,
    V4 = 4,
    V5 = 5,
    V6 = 6,
    V7 = 7,
    V8 = 8,
    V9 = 9,
    V10 = 10,
    V11 = 11,
    V12 = 12,
    V13 = 13,
    V14 = 14,
    V15 = 15,
}

impl TryFrom<u8> for Nibble {
    type Error = u8;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Ok(match value {
            0x00 => Self::V0,
            0x01 => Self::V1,
            0x02 => Self::V2,
            0x03 => Self::V3,
            0x04 => Self::V4,
            0x05 => Self::V5,
            0x06 => Self::V6,
            0x07 => Self::V7,
            0x08 => Self::V8,
            0x09 => Self::V9,
            0x0A => Self::V10,
            0x0B => Self::V11,
            0x0C => Self::V12,
            0x0D => Self::V13,
            0x0E => Self::V14,
            0x0F => Self::V15,
            x => return Err(x),
        })
    }
}

impl From<Nibble> for u8 {
    fn from(value: Nibble) -> Self {
        value as u8
    }
}

#[derive(Debug)]
pub struct NibbleIterator<I>
where
    I: Iterator<Item = u8>,
{
    inner: Peekable<I>,
    count: usize,
    next: Option<Nibble>,
}

impl<I> NibbleIterator<I>
where
    I: Iterator<Item = u8>,
{
    pub fn new(inner: impl IntoIterator<Item = u8, IntoIter = I>) -> Self {
        Self {
            inner: inner.into_iter().peekable(),
            count: 0,
            next: None,
        }
    }

    pub fn is_done(&mut self) -> bool {
        self.next.is_none() && self.inner.peek().is_none()
    }

    pub fn cmp_rest<I2>(self, rhs: NibbleIterator<I2>) -> bool
    where
        I2: Iterator<Item = u8>,
    {
        if self.count != rhs.count {
            return false;
        }

        if self.next != rhs.next {
            return false;
        }

        self.inner.eq(rhs.inner)
    }
}

impl<I> Iterator for NibbleIterator<I>
where
    I: Iterator<Item = u8>,
{
    type Item = Nibble;

    fn next(&mut self) -> Option<Self::Item> {
        self.next
            .take()
            .or_else(|| {
                self.inner.next().map(|value| {
                    self.next = Some((value & 0x0F).try_into().unwrap());
                    (value >> 4).try_into().unwrap()
                })
            })
            .map(|x| {
                self.count += 1;
                x
            })
    }

    fn nth(&mut self, n: usize) -> Option<Self::Item> {
        match n {
            0 => self.next(),
            mut n => {
                if self.next.is_some() {
                    self.next = None;
                    n -= 1;
                }

                if n >= 2 {
                    self.inner.nth(n >> 1);
                    if n % 2 == 1 {
                        self.next();
                    }
                }

                self.next()
            }
        }
    }
}
