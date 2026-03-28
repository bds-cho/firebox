use async_trait::async_trait;

use crate::{StoreError, Vm};

#[async_trait]
pub trait Store: Send + Sync + 'static {
    async fn insert(&self, vm: Vm) -> Result<(), StoreError>;
    async fn get(&self, id: &str) -> Result<Option<Vm>, StoreError>;
    async fn list(&self) -> Result<Vec<Vm>, StoreError>;
    async fn update(&self, vm: Vm) -> Result<(), StoreError>;
    async fn remove(&self, id: &str) -> Result<(), StoreError>;
}
