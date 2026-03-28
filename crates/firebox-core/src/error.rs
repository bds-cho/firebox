use firebox_store::StoreError;
use firebox_vmm::VmmError;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum CoreError {
    #[error("VM not found: {0}")]
    NotFound(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("validation error: {0}")]
    Validation(String),
    #[error("vmm error: {0}")]
    Vmm(#[from] VmmError),
    #[error("store error: {0}")]
    Store(#[from] StoreError),
    #[error("internal error: {0}")]
    Internal(String),
}
