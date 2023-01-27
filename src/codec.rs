use std::borrow::Cow;

pub trait Encode {
    fn encode(&self) -> Cow<[u8]>;
}

impl<'a> Encode for &'a [u8] {
    fn encode(&self) -> Cow<'a, [u8]> {
        Cow::Borrowed(self)
    }
}

impl Encode for Vec<u8> {
    fn encode(&self) -> Cow<[u8]> {
        Cow::Borrowed(self)
    }
}

impl<'a> Encode for &'a str {
    fn encode(&self) -> Cow<'a, [u8]> {
        Cow::Borrowed(self.as_bytes())
    }
}

impl Encode for String {
    fn encode(&self) -> Cow<[u8]> {
        Cow::Borrowed(self.as_bytes())
    }
}

impl<'a, const N: usize> Encode for &'a [u8; N] {
    fn encode(&self) -> Cow<'a, [u8]> {
        Cow::Borrowed(self.as_slice())
    }
}

impl<const N: usize> Encode for [u8; N] {
    fn encode(&self) -> Cow<[u8]> {
        Cow::Borrowed(self.as_slice())
    }
}
