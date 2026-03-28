mod stub;
mod firecracker;

pub use stub::StubVmm;
pub use firecracker::FirecrackerVmm;

use async_trait::async_trait;
use firebox_store::Vm;
use thiserror::Error;

#[derive(Debug)]
pub struct SpawnResult {
    pub pid: u32,
}

#[derive(Debug, Error)]
pub enum VmmError {
    #[error("spawn failed: {0}")]
    SpawnFailed(String),
    #[error("kill failed: {0}")]
    KillFailed(String),
}

#[async_trait]
pub trait Vmm: Send + Sync + 'static {
    /// Spawn a VM process. `socket_path` is the Firecracker Unix socket path.
    async fn spawn(&self, vm: &Vm, socket_path: &str) -> Result<SpawnResult, VmmError>;

    /// Forcefully stop a running VM by PID.
    async fn kill(&self, pid: u32) -> Result<(), VmmError>;
}
