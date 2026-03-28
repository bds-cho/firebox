use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::RwLock;

use crate::{Store, StoreError, Vm};

#[derive(Clone, Default)]
pub struct MemoryStore {
    inner: Arc<RwLock<HashMap<String, Vm>>>,
}

impl MemoryStore {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Store for MemoryStore {
    async fn insert(&self, vm: Vm) -> Result<(), StoreError> {
        let mut map = self.inner.write().await;
        if map.contains_key(&vm.id) {
            return Err(StoreError::AlreadyExists(vm.id));
        }
        map.insert(vm.id.clone(), vm);
        Ok(())
    }

    async fn get(&self, id: &str) -> Result<Option<Vm>, StoreError> {
        let map = self.inner.read().await;
        Ok(map.get(id).cloned())
    }

    async fn list(&self) -> Result<Vec<Vm>, StoreError> {
        let map = self.inner.read().await;
        Ok(map.values().cloned().collect())
    }

    async fn update(&self, vm: Vm) -> Result<(), StoreError> {
        let mut map = self.inner.write().await;
        map.insert(vm.id.clone(), vm);
        Ok(())
    }

    async fn remove(&self, id: &str) -> Result<(), StoreError> {
        let mut map = self.inner.write().await;
        map.remove(id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{VmStatus};

    fn make_vm(id: &str) -> Vm {
        Vm {
            id: id.to_string(),
            vcpus: 1,
            memory_mb: 128,
            kernel: "/boot/vmlinux".to_string(),
            rootfs: "/var/rootfs.ext4".to_string(),
            network: None,
            status: VmStatus::Created,
            pid: None,
        }
    }

    #[tokio::test]
    async fn insert_and_get() {
        let store = MemoryStore::new();
        let vm = make_vm("vm-1");
        store.insert(vm.clone()).await.unwrap();
        let got = store.get("vm-1").await.unwrap().unwrap();
        assert_eq!(got.id, "vm-1");
    }

    #[tokio::test]
    async fn insert_duplicate_fails() {
        let store = MemoryStore::new();
        store.insert(make_vm("vm-1")).await.unwrap();
        assert!(store.insert(make_vm("vm-1")).await.is_err());
    }

    #[tokio::test]
    async fn get_missing_returns_none() {
        let store = MemoryStore::new();
        assert!(store.get("nope").await.unwrap().is_none());
    }

    #[tokio::test]
    async fn list_returns_all() {
        let store = MemoryStore::new();
        store.insert(make_vm("a")).await.unwrap();
        store.insert(make_vm("b")).await.unwrap();
        let list = store.list().await.unwrap();
        assert_eq!(list.len(), 2);
    }

    #[tokio::test]
    async fn update_overwrites() {
        let store = MemoryStore::new();
        store.insert(make_vm("vm-1")).await.unwrap();
        let mut vm = make_vm("vm-1");
        vm.status = VmStatus::Running;
        store.update(vm).await.unwrap();
        let got = store.get("vm-1").await.unwrap().unwrap();
        assert_eq!(got.status, VmStatus::Running);
    }

    #[tokio::test]
    async fn remove_deletes() {
        let store = MemoryStore::new();
        store.insert(make_vm("vm-1")).await.unwrap();
        store.remove("vm-1").await.unwrap();
        assert!(store.get("vm-1").await.unwrap().is_none());
    }
}
