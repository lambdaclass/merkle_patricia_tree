#[cfg(all(unix))]
mod storage_mdbx {
    pub use self::error::Result;
    use digest::{Digest, Output};
    use libmdbx::{Database, Geometry, NoWriteMap, WriteFlags};
    use patricia_merkle_tree::{Encode, PatriciaMerkleTree};
    use rand::{rngs::StdRng, RngCore, SeedableRng};
    use serde::{Deserialize, Serialize};
    use sha3::Keccak256;
    use std::{borrow::Cow, marker::PhantomData, path::Path, rc::Rc};
    use tempfile::tempdir;
    use uuid::Uuid;

    pub mod error {
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
            let db = Database::new()
                .set_geometry(Geometry {
                    size: Some(0..1024 * 1024 * 64),
                    ..Default::default()
                })
                .open(storage_path.as_ref())?;

            Ok(Self {
                tree: PatriciaMerkleTree::new(),
                db: Rc::new(db),
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

    pub fn run() -> Result<()> {
        let temp_dir = tempdir()?;
        let mut tree = MdbxStorageTree::<[u8; 32], [u8; 32], Keccak256>::new(temp_dir.path())?;

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
}

#[cfg(all(unix))]
fn main() {
    storage_mdbx::run().unwrap();
}

#[cfg(not(unix))]
fn main() {
    eprintln!("this example only works on unix-like operating systems.")
}
