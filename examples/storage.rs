use self::error::Result;
use digest::{Digest, Output};
use patricia_merkle_tree::{Encode, PatriciaMerkleTree};
use serde::{Deserialize, Serialize};
use sha3::Keccak256;
use std::{
    borrow::Cow,
    fs::{remove_file, File},
    io::{BufReader, BufWriter},
    marker::PhantomData,
    path::{Path, PathBuf},
    rc::Rc,
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

struct StorageRef<P, V, H>(pub Rc<PathBuf>, pub Uuid, pub PhantomData<(P, V, H)>)
where
    P: Encode,
    V: Encode + Serialize + for<'de> Deserialize<'de>,
    H: Digest;

impl<P, V, H> Encode for StorageRef<P, V, H>
where
    P: Encode,
    V: Encode + Serialize + for<'de> Deserialize<'de>,
    H: Digest,
{
    fn encode(&self) -> Cow<[u8]> {
        let value: V = StorageTree::<P, V, H>::load_value(&self.0, &self.1).unwrap();
        Cow::Owned(value.encode().into_owned())
    }
}

struct StorageTree<P, V, H>
where
    P: Encode,
    V: Encode + Serialize + for<'de> Deserialize<'de>,
    H: Digest,
{
    tree: PatriciaMerkleTree<P, StorageRef<P, V, H>, H>,
    storage_path: Rc<PathBuf>,

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
            storage_path: Rc::new(storage_path.into()),
            phantom: PhantomData,
        }
    }

    pub fn get(&self, path: &P) -> Result<Option<V>> {
        self.tree
            .get(path)
            .map(|storage_key| Self::load_value(&self.storage_path, &storage_key.1))
            .transpose()
    }

    pub fn insert(&mut self, path: P, value: V) -> Result<Option<V>> {
        let storage_key = Self::store_value(&self.storage_path, value)?;
        self.tree
            .insert(
                path,
                StorageRef(Rc::clone(&self.storage_path), storage_key, PhantomData),
            )
            .map(|storage_key| {
                let value = Self::load_value(&self.storage_path, &storage_key.1)?;
                Self::erase_value(&self.storage_path, &storage_key.1)?;
                Ok(value)
            })
            .transpose()
    }

    pub fn compute_hash(&mut self) -> &Output<H> {
        self.tree.compute_hash()
    }

    fn load_value(storage_path: &Path, storage_key: &Uuid) -> Result<V> {
        let file = File::open(
            storage_path
                .join(storage_key.to_string())
                .with_extension("json"),
        )?;
        let reader = BufReader::new(file);

        serde_json::from_reader(reader).map_err(Into::into)
    }

    fn erase_value(storage_path: &Path, storage_key: &Uuid) -> Result<()> {
        remove_file(
            storage_path
                .join(storage_key.to_string())
                .with_extension("json"),
        )?;
        Ok(())
    }

    fn store_value(storage_path: &Path, value: V) -> Result<Uuid> {
        let (storage_key, path) = loop {
            let storage_key = Uuid::new_v4();
            let path = storage_path
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
    println!("root hash is {:02x?}", tree.compute_hash());

    Ok(())
}
