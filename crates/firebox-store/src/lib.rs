mod memory;
mod store;

pub use memory::MemoryStore;
pub use store::Store;

use serde::{Deserialize, Serialize};
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum VmStatus {
    Created,
    Running,
    Stopped,
}

impl std::fmt::Display for VmStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            VmStatus::Created => write!(f, "created"),
            VmStatus::Running => write!(f, "running"),
            VmStatus::Stopped => write!(f, "stopped"),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct NetworkConfig {
    pub tap_device: String,
    pub mac: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Vm {
    pub id: String,
    pub vcpus: u8,
    pub memory_mb: u32,
    pub kernel: String,
    pub rootfs: String,
    pub network: Option<NetworkConfig>,
    pub status: VmStatus,
    pub pid: Option<u32>,
}

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("VM already exists: {0}")]
    AlreadyExists(String),
    #[error("internal store error: {0}")]
    Internal(String),
}
