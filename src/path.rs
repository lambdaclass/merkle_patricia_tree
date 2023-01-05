use std::io::{self, Write};

pub trait TreePath {
    type Path: Eq;

    /// Return the path for the given value.
    fn path(&self) -> Self::Path;

    /// Redirect to `Self::encode_path` when encoding its own path.
    fn encode_self_path(&self, target: impl Write) -> io::Result<()> {
        Self::encode_path(&self.path(), target)
    }

    /// Encode the path to be used within the tree.
    ///
    /// > Note: When using rlp encoding, encode the path here.
    fn encode_path(path: &Self::Path, target: impl Write) -> io::Result<()>;
    // fn encode_path(path: &Self::Path, target: &mut BytesMut) {
    //     target.extend_from_slice(path.as_ref());
    // }
}
