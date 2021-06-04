use crate::{plumbing::DatabaseStorageTypes, Runtime};
use std::sync::Arc;

/// Stores the cached results and dependency information for all the queries
/// defined on your salsa database. Also embeds a [`Runtime`] which is used to
/// manage query execution. Every database must include a `storage:
/// Storage<Self>` field.
pub struct Storage<DB: DatabaseStorageTypes> {
    global_storage: Arc<DB::DatabaseGlobalStorage>,
    query_store: Arc<DB::DatabaseStorage>,
    runtime: Runtime,
}

impl<DB: DatabaseStorageTypes> Default for Storage<DB> {
    fn default() -> Self {
        Self {
            global_storage: Default::default(),
            query_store: Default::default(),
            runtime: Default::default(),
        }
    }
}

impl<DB: DatabaseStorageTypes> Storage<DB> {
    /// Gives access to the underlying salsa runtime.
    pub fn salsa_runtime(&self) -> &Runtime {
        &self.runtime
    }

    /// Gives access to the underlying salsa runtime.
    pub fn salsa_runtime_mut(&mut self) -> &mut Runtime {
        &mut self.runtime
    }

    /// Access the query storage tables. Not meant to be used directly by end
    /// users.
    pub fn global_storage(&self) -> &DB::DatabaseGlobalStorage {
        &self.global_storage
    }

    /// Access the query storage tables. Not meant to be used directly by end
    /// users.
    pub fn query_store(&self) -> &DB::DatabaseStorage {
        &self.query_store
    }

    /// Returns a "snapshotted" storage, suitable for use in a forked database.
    /// This snapshot hold a read-lock on the global state, which means that any
    /// attempt to `set` an input will block until the forked runtime is
    /// dropped. See `ParallelDatabase::snapshot` for more information.
    ///
    /// **Warning.** This second handle is intended to be used from a separate
    /// thread. Using two database handles from the **same thread** can lead to
    /// deadlock.
    pub fn snapshot(&self) -> Self {
        Storage {
            global_storage: self.global_storage.clone(),
            query_store: self.query_store.clone(),
            runtime: self.runtime.snapshot(),
        }
    }
}
