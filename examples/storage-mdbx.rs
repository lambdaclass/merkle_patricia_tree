use self::error::Result;
use digest::{Digest, Output};
use libmdbx::{Database, NoWriteMap, WriteFlags};
use patricia_merkle_tree::{Encode, PatriciaMerkleTree};
use serde::{Deserialize, Serialize};
use sha3::Keccak256;
use std::{borrow::Cow, marker::PhantomData, path::Path, rc::Rc};
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
        #[error(transparent)]
        Bincode(#[from] bincode::Error),
        #[error(transparent)]
        Mdbx(#[from] libmdbx::Error),
    }
}

type TreeDB = Database<NoWriteMap>;

struct StorageRef<P, V, H>(pub Rc<TreeDB>, pub Uuid, pub PhantomData<(P, V, H)>)
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
        let value: V = MdbxStorageTree::<P, V, H>::load_value(&self.0, &self.1).unwrap();
        Cow::Owned(value.encode().into_owned())
    }
}

struct MdbxStorageTree<P, V, H>
where
    P: Encode,
    V: Encode + Serialize + for<'de> Deserialize<'de>,
    H: Digest,
{
    tree: PatriciaMerkleTree<P, StorageRef<P, V, H>, H>,
    db: Rc<Database<NoWriteMap>>,
}

impl<P, V, H> MdbxStorageTree<P, V, H>
where
    P: Encode,
    V: Encode + Serialize,
    for<'de> V: Deserialize<'de>,
    H: Digest,
{
    pub fn new<T: AsRef<Path>>(storage_path: T) -> Result<Self> {
        Ok(Self {
            tree: PatriciaMerkleTree::new(),
            db: Rc::new(Database::new().open(storage_path.as_ref())?),
        })
    }

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

    fn load_value(db: &TreeDB, storage_key: &Uuid) -> Result<V> {
        let tx = db.begin_ro_txn()?;
        let table = tx.open_table(None)?;

        let value: Cow<[u8]> = tx
            .get(&table, storage_key.as_bytes().as_slice())?
            .expect("value to be there");

        bincode::deserialize(&value).map_err(Into::into)
    }

    fn erase_value(db: &TreeDB, storage_key: &Uuid) -> Result<()> {
        let tx = db.begin_rw_txn()?;
        let table = tx.open_table(None)?;
        tx.del(&table, storage_key, None)?;
        tx.commit()?;
        Ok(())
    }

    fn store_value(db: &TreeDB, value: V) -> Result<Uuid> {
        let storage_key = Uuid::new_v4();
        let value = bincode::serialize(&value)?;

        let tx = db.begin_rw_txn()?;
        let table = tx.open_table(None)?;
        tx.put(&table, storage_key, value, WriteFlags::empty())?;
        tx.commit()?;

        Ok(storage_key)
    }
}

fn main() -> Result<()> {
    let temp_dir = tempdir()?;
    let mut tree = MdbxStorageTree::<Vec<_>, Vec<_>, Keccak256>::new(temp_dir.path())?;

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
