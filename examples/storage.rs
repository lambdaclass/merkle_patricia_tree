use self::error::Result;
use digest::Digest;
use patricia_merkle_tree::{Encode, PatriciaMerkleTree};
use serde::{Deserialize, Serialize};
use sha3::Keccak256;
use std::{
    borrow::Cow,
    fs::{remove_file, File},
    io::{BufReader, BufWriter},
    marker::PhantomData,
    path::PathBuf,
};
use tempfile::tempdir;
use uuid::Uuid;

mod error {
    use thiserror::Error;

    pub type Result<T> = std::result::Result<T, Error>;

    #[derive(Debug, Error)]
    pub enum Error {
        #[error(transparent)]
        Io(#[from] std::io::Error),
        #[error(transparent)]
        Json(#[from] serde_json::Error),
    }
}

struct UuidPath(pub Uuid);

impl Encode for UuidPath {
    fn encode(&self) -> Cow<[u8]> {
        Cow::Owned(self.0.to_string().into_bytes())
    }
}

struct StorageTree<P, V, H>
where
    P: Encode,
    V: Encode + Serialize,
    for<'de> V: Deserialize<'de>,
    H: Digest,
{
    tree: PatriciaMerkleTree<P, UuidPath, H>,
    storage_path: PathBuf,

    phantom: PhantomData<V>,
}

impl<P, V, H> StorageTree<P, V, H>
where
    P: Encode,
    V: Encode + Serialize,
    for<'de> V: Deserialize<'de>,
    H: Digest,
{
    pub fn new(storage_path: impl Into<PathBuf>) -> Self {
        Self {
            tree: PatriciaMerkleTree::new(),
            storage_path: storage_path.into(),
            phantom: PhantomData,
        }
    }

    pub fn get(&self, path: &P) -> Result<Option<V>> {
        self.tree
            .get(path)
            .map(|storage_key| self.load_value(&storage_key.0))
            .transpose()
    }

    pub fn insert(&mut self, path: P, value: V) -> Result<Option<V>> {
        let storage_key = self.store_value(value)?;
        self.tree
            .insert(path, UuidPath(storage_key))
            .map(|storage_key| {
                let value = self.load_value(&storage_key.0)?;
                self.erase_value(&storage_key.0)?;
                Ok(value)
            })
            .transpose()
    }

    pub fn compute_hash(&mut self) {
        todo!()
    }

    fn load_value(&self, storage_key: &Uuid) -> Result<V> {
        let file = File::open(
            self.storage_path
                .join(storage_key.to_string())
                .with_extension("json"),
        )?;
        let reader = BufReader::new(file);

        serde_json::from_reader(reader).map_err(Into::into)
    }

    fn erase_value(&self, storage_key: &Uuid) -> Result<()> {
        remove_file(
            self.storage_path
                .join(storage_key.to_string())
                .with_extension("json"),
        )?;
        Ok(())
    }

    fn store_value(&self, value: V) -> Result<Uuid> {
        let (storage_key, path) = loop {
            let storage_key = Uuid::new_v4();
            let path = self
                .storage_path
                .join(storage_key.to_string())
                .with_extension("json");

            if !path.exists() {
                break (storage_key, path);
            }
        };

        let file = File::create(path)?;
        let writer = BufWriter::new(file);

        serde_json::to_writer(writer, &value)?;
        Ok(storage_key)
    }
}

fn main() -> Result<()> {
    let temp_dir = tempdir()?;
    let mut tree = StorageTree::<Vec<_>, Vec<_>, Keccak256>::new(temp_dir.path());

    let (path_a, node_a) = (vec![0x12], vec![1]);
    let (path_b, node_b) = (vec![0x34], vec![2]);
    let (path_c, node_c) = (vec![0x56], vec![3]);

    tree.insert(path_a, node_a)?;
    tree.insert(path_b, node_b)?;
    tree.insert(path_c, node_c)?;

    assert_eq!(tree.get(&vec![0x12])?, Some(vec![1]));
    assert_eq!(tree.get(&vec![0x34])?, Some(vec![2]));
    assert_eq!(tree.get(&vec![0x56])?, Some(vec![3]));

    Ok(())
}
