use async_trait::async_trait;
use firebox_store::Vm;
use tracing::info;

use crate::{SpawnResult, Vmm, VmmError};

/// No-op VMM for MVP. Returns a fake PID so the full lifecycle can be exercised.
/// The fake PID (99999) is unlikely to exist, so lazy PID reconciliation in Core
/// will correctly detect it as dead and transition the VM to Stopped.
pub struct StubVmm;

#[async_trait]
impl Vmm for StubVmm {
    async fn spawn(&self, vm: &Vm, socket_path: &str) -> Result<SpawnResult, VmmError> {
        info!(vm_id = %vm.id, socket_path, "StubVmm: spawn (no-op)");
        Ok(SpawnResult { pid: 99999 })
    }

    async fn kill(&self, pid: u32) -> Result<(), VmmError> {
        info!(pid, "StubVmm: kill (no-op)");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use firebox_store::{Vm, VmStatus};

    fn make_vm() -> Vm {
        Vm {
            id: "test-vm".to_string(),
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
    async fn spawn_returns_fake_pid() {
        let vmm = StubVmm;
        let result = vmm.spawn(&make_vm(), "/run/firebox/test-vm.sock").await.unwrap();
        assert_eq!(result.pid, 99999);
    }

    #[tokio::test]
    async fn kill_succeeds() {
        let vmm = StubVmm;
        vmm.kill(99999).await.unwrap();
    }
}
