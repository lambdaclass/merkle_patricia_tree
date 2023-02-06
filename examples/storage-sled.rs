//! Example of a Storage implementation using Sled as the database and bincode to encode the saved values.
use digest::{Digest, Output};
use error::Result;
use patricia_merkle_tree::{Encode, PatriciaMerkleTree};
use rand::{rngs::StdRng, RngCore, SeedableRng};
use serde::{Deserialize, Serialize};
use sha3::Keccak256;
use sled::Db;
use std::{borrow::Cow, marker::PhantomData, path::Path};
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
        Bincode(#[from] bincode::Error),
        #[error(transparent)]
        Sled(#[from] sled::Error),
    }
}

struct StorageRef<P, V, H>(pub Db, pub Uuid, pub PhantomData<(P, V, H)>)
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
        let value: V = SledStorageTree::<P, V, H>::load_value(&self.0, &self.1).unwrap();
        Cow::Owned(value.encode().into_owned())
    }
}

struct SledStorageTree<P, V, H>
where
    P: Encode,
    V: Encode + Serialize + for<'de> Deserialize<'de>,
    H: Digest,
{
    tree: PatriciaMerkleTree<P, StorageRef<P, V, H>, H>,
    db: Db,
}

impl<P, V, H> SledStorageTree<P, V, H>
where
    P: Encode,
    V: Encode + Serialize,
    for<'de> V: Deserialize<'de>,
    H: Digest,
{
    pub fn new<T: AsRef<Path>>(storage_path: T) -> Result<Self> {
        Ok(Self {
            tree: PatriciaMerkleTree::new(),
            db: sled::open(storage_path)?,
        })
    }

    #[allow(dead_code)]
    pub fn get(&self, path: &P) -> Result<Option<V>> {
        self.tree
            .get(path)
            .map(|storage_key| Self::load_value(&self.db, &storage_key.1))
            .transpose()
    }

    pub fn insert(&mut self, path: P, value: V) -> Result<Option<V>> {
        let storage_key = Self::store_value(&self.db, value)?;
        self.tree
            .insert(path, StorageRef(self.db.clone(), storage_key, PhantomData))
            .map(|storage_key| {
                let value = Self::load_value(&self.db, &storage_key.1)?;
                Self::erase_value(&self.db, &storage_key.1)?;
                Ok(value)
            })
            .transpose()
    }

    pub fn compute_hash(&mut self) -> &Output<H> {
        self.tree.compute_hash()
    }

    fn load_value(db: &Db, storage_key: &Uuid) -> Result<V> {
        let value = db.get(storage_key)?;
        bincode::deserialize(&value.unwrap()).map_err(Into::into)
    }

    fn erase_value(db: &Db, storage_key: &Uuid) -> Result<()> {
        db.remove(storage_key)?;
        Ok(())
    }

    fn store_value(db: &Db, value: V) -> Result<Uuid> {
        let storage_key = Uuid::new_v4();
        let value = bincode::serialize(&value)?;
        db.insert(storage_key, value)?;
        Ok(storage_key)
    }
}

fn main() -> Result<()> {
    let temp_dir = tempdir()?;
    let mut tree = SledStorageTree::<[u8; 32], [u8; 32], Keccak256>::new(temp_dir.path())?;

    let n = std::env::args().nth(1).expect("missing number of nodes");
    let n: usize = n.parse().expect("valid number");

    let mut rng = StdRng::seed_from_u64(1234);
    let mut key = [0u8; 32];
    let mut value = [0u8; 32];

    for _ in 0..n {
        rng.fill_bytes(&mut key);
        rng.fill_bytes(&mut value);
        tree.insert(key, value)?;
    }
    println!("root hash is {:02x?}", tree.compute_hash());

    Ok(())
}
