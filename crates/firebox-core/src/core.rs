use std::sync::Arc;

use firebox_store::{Store, Vm, VmStatus};
use firebox_vmm::Vmm;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::{DaemonConfig, VmConfig};
use crate::error::CoreError;

pub struct Core<S: Store, V: Vmm> {
    store: Arc<S>,
    vmm: Arc<V>,
    config: Arc<DaemonConfig>,
}

impl<S: Store, V: Vmm> Core<S, V> {
    pub fn new(store: Arc<S>, vmm: Arc<V>, config: Arc<DaemonConfig>) -> Self {
        Self { store, vmm, config }
    }

    fn socket_path(&self, vm_id: &str) -> String {
        format!("{}/{}.sock", self.config.socket_dir, vm_id)
    }

    fn validate(config: &VmConfig) -> Result<(), CoreError> {
        if config.vcpus == 0 {
            return Err(CoreError::Validation("vcpus must be >= 1".to_string()));
        }
        if config.memory_mb < 128 {
            return Err(CoreError::Validation("memory_mb must be >= 128".to_string()));
        }
        if config.kernel.is_empty() {
            return Err(CoreError::Validation("kernel path is required".to_string()));
        }
        if config.rootfs.is_empty() {
            return Err(CoreError::Validation("rootfs path is required".to_string()));
        }
        Ok(())
    }

    pub async fn create_vm(&self, config: VmConfig) -> Result<Vm, CoreError> {
        Self::validate(&config)?;

        let kernel = std::fs::canonicalize(&config.kernel)
            .map_err(|e| CoreError::Validation(format!("invalid kernel path: {e}")))?
            .to_string_lossy()
            .to_string();
        let rootfs = std::fs::canonicalize(&config.rootfs)
            .map_err(|e| CoreError::Validation(format!("invalid rootfs path: {e}")))?
            .to_string_lossy()
            .to_string();

        let id = config.id.unwrap_or_else(|| Uuid::new_v4().to_string());
        let vm = Vm {
            id: id.clone(),
            vcpus: config.vcpus,
            memory_mb: config.memory_mb,
            kernel,
            rootfs,
            network: config.network,
            status: VmStatus::Created,
            pid: None,
        };

        self.store.insert(vm.clone()).await?;
        info!(vm_id = %id, "VM created");
        Ok(vm)
    }

    pub async fn get_vm(&self, id: &str) -> Result<Vm, CoreError> {
        let mut vm = self
            .store
            .get(id)
            .await?
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;

        // Lazily reconcile: if process is dead, transition to Stopped.
        if vm.status == VmStatus::Running {
            if let Some(pid) = vm.pid {
                let alive = unsafe { libc::kill(pid as libc::pid_t, 0) } == 0;
                if !alive {
                    warn!(vm_id = %id, pid, "VM process not found, marking stopped");
                    vm.status = VmStatus::Stopped;
                    vm.pid = None;
                    self.store.update(vm.clone()).await?;
                }
            }
        }

        Ok(vm)
    }

    pub async fn list_vms(&self) -> Result<Vec<Vm>, CoreError> {
        Ok(self.store.list().await?)
    }

    pub async fn start_vm(&self, id: &str) -> Result<Vm, CoreError> {
        let vm = self
            .store
            .get(id)
            .await?
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;

        if vm.status == VmStatus::Running {
            return Err(CoreError::Conflict(format!("VM {id} is already running")));
        }

        let socket_path = self.socket_path(id);
        let spawn_result = self.vmm.spawn(&vm, &socket_path).await?;

        let mut updated = vm;
        updated.status = VmStatus::Running;
        updated.pid = Some(spawn_result.pid);
        self.store.update(updated.clone()).await?;
        info!(vm_id = %id, pid = spawn_result.pid, "VM started");
        Ok(updated)
    }

    pub async fn stop_vm(&self, id: &str) -> Result<Vm, CoreError> {
        let vm = self
            .store
            .get(id)
            .await?
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;

        if vm.status != VmStatus::Running {
            return Err(CoreError::Conflict(format!("VM {id} is not running")));
        }

        if let Some(pid) = vm.pid {
            self.vmm.kill(pid).await?;
        }

        let socket_path = self.socket_path(id);
        let _ = std::fs::remove_file(&socket_path);

        let mut updated = vm;
        updated.status = VmStatus::Stopped;
        updated.pid = None;
        self.store.update(updated.clone()).await?;
        info!(vm_id = %id, "VM stopped");
        Ok(updated)
    }

    pub async fn delete_vm(&self, id: &str) -> Result<(), CoreError> {
        let vm = self
            .store
            .get(id)
            .await?
            .ok_or_else(|| CoreError::NotFound(id.to_string()))?;

        if vm.status == VmStatus::Running {
            return Err(CoreError::Conflict(format!(
                "VM {id} is running, stop it before deleting"
            )));
        }

        self.store.remove(id).await?;
        info!(vm_id = %id, "VM deleted");
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use firebox_store::MemoryStore;
    use firebox_vmm::StubVmm;

    fn make_core() -> Core<MemoryStore, StubVmm> {
        Core::new(
            Arc::new(MemoryStore::new()),
            Arc::new(StubVmm),
            Arc::new(DaemonConfig::default()),
        )
    }

    fn vm_config(id: &str) -> VmConfig {
        VmConfig {
            id: Some(id.to_string()),
            vcpus: 1,
            memory_mb: 128,
            kernel: "/boot/vmlinux".to_string(),
            rootfs: "/var/rootfs.ext4".to_string(),
            network: None,
        }
    }

    #[tokio::test]
    async fn create_and_get() {
        let core = make_core();
        let vm = core.create_vm(vm_config("vm-1")).await.unwrap();
        assert_eq!(vm.id, "vm-1");
        assert_eq!(vm.status, VmStatus::Created);

        let got = core.get_vm("vm-1").await.unwrap();
        assert_eq!(got.id, "vm-1");
    }

    #[tokio::test]
    async fn get_missing_returns_not_found() {
        let core = make_core();
        assert!(matches!(core.get_vm("nope").await, Err(CoreError::NotFound(_))));
    }

    #[tokio::test]
    async fn full_lifecycle() {
        let core = make_core();
        core.create_vm(vm_config("vm-1")).await.unwrap();

        let vm = core.start_vm("vm-1").await.unwrap();
        assert_eq!(vm.status, VmStatus::Running);
        assert_eq!(vm.pid, Some(99999));

        let vm = core.stop_vm("vm-1").await.unwrap();
        assert_eq!(vm.status, VmStatus::Stopped);
        assert_eq!(vm.pid, None);

        core.delete_vm("vm-1").await.unwrap();
        assert!(matches!(core.get_vm("vm-1").await, Err(CoreError::NotFound(_))));
    }

    #[tokio::test]
    async fn start_running_vm_is_conflict() {
        let core = make_core();
        core.create_vm(vm_config("vm-1")).await.unwrap();
        core.start_vm("vm-1").await.unwrap();
        assert!(matches!(core.start_vm("vm-1").await, Err(CoreError::Conflict(_))));
    }

    #[tokio::test]
    async fn stop_non_running_vm_is_conflict() {
        let core = make_core();
        core.create_vm(vm_config("vm-1")).await.unwrap();
        assert!(matches!(core.stop_vm("vm-1").await, Err(CoreError::Conflict(_))));
    }

    #[tokio::test]
    async fn delete_running_vm_is_conflict() {
        let core = make_core();
        core.create_vm(vm_config("vm-1")).await.unwrap();
        core.start_vm("vm-1").await.unwrap();
        assert!(matches!(core.delete_vm("vm-1").await, Err(CoreError::Conflict(_))));
    }

    #[tokio::test]
    async fn validation_rejects_zero_vcpus() {
        let core = make_core();
        let cfg = VmConfig { vcpus: 0, ..vm_config("vm-1") };
        assert!(matches!(core.create_vm(cfg).await, Err(CoreError::Validation(_))));
    }

    #[tokio::test]
    async fn validation_rejects_low_memory() {
        let core = make_core();
        let cfg = VmConfig { memory_mb: 64, ..vm_config("vm-1") };
        assert!(matches!(core.create_vm(cfg).await, Err(CoreError::Validation(_))));
    }

    #[tokio::test]
    async fn list_returns_all_vms() {
        let core = make_core();
        core.create_vm(vm_config("vm-1")).await.unwrap();
        core.create_vm(vm_config("vm-2")).await.unwrap();
        let vms = core.list_vms().await.unwrap();
        assert_eq!(vms.len(), 2);
    }
}
