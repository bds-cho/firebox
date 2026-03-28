# FirecrackerVmm Code Outline

## File Structure

```
crates/firebox-vmm/
├── src/
│   ├── lib.rs          (re-exports: Vmm, StubVmm, FirecrackerVmm)
│   ├── stub.rs         (StubVmm - existing, unchanged)
│   └── firecracker.rs  (NEW - FirecrackerVmm implementation)
└── Cargo.toml
```

## Cargo.toml

```toml
[package]
name = "firebox-vmm"
version = "0.1.0"
edition = "2021"

[dependencies]
firebox-store = { workspace = true }
async-trait   = { workspace = true }
thiserror     = { workspace = true }
tracing       = { workspace = true }
tokio         = { workspace = true }
reqwest       = { version = "0.12", features = ["json"] }
hyperlocal    = "0.1"
serde         = { workspace = true }
serde_json    = { workspace = true }

[dev-dependencies]
tokio = { workspace = true }
```

## src/firecracker.rs - Full Implementation

```rust
use std::path::Path;
use std::process::Command;
use std::time::Duration;

use async_trait::async_trait;
use firebox_store::Vm;
use hyperlocal::UnixConnector;
use reqwest::Client;
use serde::Serialize;
use thiserror::Error;
use tokio::time::sleep;
use tracing::{info, warn};

use crate::{SpawnResult, Vmm, VmmError};

/// Firecracker HTTP socket API config structs
#[derive(Serialize)]
struct MachineConfig {
    vcpu_count: u8,
    mem_size_mib: u32,
}

#[derive(Serialize)]
struct BootSource {
    kernel_image_path: String,
    boot_args: String,
}

#[derive(Serialize)]
struct DriveConfig {
    drive_id: String,
    path_on_host: String,
    is_root_device: bool,
    is_read_only: bool,
}

#[derive(Serialize)]
struct NetworkInterface {
    iface_id: String,
    host_dev_name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    guest_mac: Option<String>,
}

#[derive(Serialize)]
struct InstanceAction {
    action_type: String,
}

/// Real Firecracker VMM implementation
pub struct FirecrackerVmm {
    firecracker_bin: String,
}

impl FirecrackerVmm {
    pub fn new(firecracker_bin: String) -> Self {
        Self { firecracker_bin }
    }

    /// Wait for Firecracker socket to be ready (file exists)
    async fn wait_for_socket(socket_path: &str, timeout: Duration) -> Result<(), VmmError> {
        let start = std::time::Instant::now();
        loop {
            if Path::new(socket_path).exists() {
                info!(socket_path, "Socket ready");
                return Ok(());
            }
            if start.elapsed() > timeout {
                return Err(VmmError::SpawnFailed(
                    format!("socket not ready after {:?}", timeout),
                ));
            }
            sleep(Duration::from_millis(100)).await;
        }
    }

    /// Configure VM via Firecracker HTTP socket API
    async fn configure_vm(socket_path: &str, vm: &Vm) -> Result<(), VmmError> {
        let client = Client::builder()
            .connector(UnixConnector)
            .build()
            .map_err(|e| VmmError::SpawnFailed(format!("http client failed: {}", e)))?;

        // Construct base URI for this socket
        // Format: http+unix://socket-path/endpoint
        // For socket at /run/firebox/sockets/vm-1.sock, use:
        // http+unix:///run/firebox/sockets/vm-1.sock/machine-config
        let base = format!("http+unix://{}", socket_path);

        // 1. Machine config
        let machine_cfg = MachineConfig {
            vcpu_count: vm.vcpus,
            mem_size_mib: vm.memory_mb,
        };
        let resp = client
            .put(format!("{}/machine-config", base))
            .json(&machine_cfg)
            .send()
            .await
            .map_err(|e| VmmError::SpawnFailed(format!("machine-config failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(VmmError::SpawnFailed(
                format!("machine-config: HTTP {}", resp.status()),
            ));
        }

        // 2. Boot source (kernel + hardcoded boot args)
        let boot_cfg = BootSource {
            kernel_image_path: vm.kernel.clone(),
            boot_args: "console=ttyS0 reboot=k panic=1 pci=off".to_string(),
        };
        let resp = client
            .put(format!("{}/boot-source", base))
            .json(&boot_cfg)
            .send()
            .await
            .map_err(|e| VmmError::SpawnFailed(format!("boot-source failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(VmmError::SpawnFailed(format!(
                "boot-source: HTTP {}",
                resp.status()
            )));
        }

        // 3. Root drive (rootfs)
        let drive_cfg = DriveConfig {
            drive_id: "rootfs".to_string(),
            path_on_host: vm.rootfs.clone(),
            is_root_device: true,
            is_read_only: false,
        };
        let resp = client
            .put(format!("{}/drives/rootfs", base))
            .json(&drive_cfg)
            .send()
            .await
            .map_err(|e| VmmError::SpawnFailed(format!("drives/rootfs failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(VmmError::SpawnFailed(format!(
                "drives/rootfs: HTTP {}",
                resp.status()
            )));
        }

        // 4. Network interface (if configured)
        if let Some(ref net_cfg) = vm.network {
            let net_iface = NetworkInterface {
                iface_id: "eth0".to_string(),
                host_dev_name: net_cfg.tap_device.clone(),
                guest_mac: net_cfg.mac.clone(),
            };
            let resp = client
                .put(format!("{}/network-interfaces/eth0", base))
                .json(&net_iface)
                .send()
                .await
                .map_err(|e| {
                    VmmError::SpawnFailed(format!("network-interfaces/eth0 failed: {}", e))
                })?;

            if !resp.status().is_success() {
                return Err(VmmError::SpawnFailed(format!(
                    "network-interfaces/eth0: HTTP {}",
                    resp.status()
                )));
            }
        }

        // 5. Start the VM
        let action = InstanceAction {
            action_type: "InstanceStart".to_string(),
        };
        let resp = client
            .put(format!("{}/actions", base))
            .json(&action)
            .send()
            .await
            .map_err(|e| VmmError::SpawnFailed(format!("actions failed: {}", e)))?;

        if !resp.status().is_success() {
            return Err(VmmError::SpawnFailed(format!(
                "actions: HTTP {}",
                resp.status()
            )));
        }

        info!("VM configured and started successfully");
        Ok(())
    }
}

#[async_trait]
impl Vmm for FirecrackerVmm {
    async fn spawn(&self, vm: &Vm, socket_path: &str) -> Result<SpawnResult, VmmError> {
        info!(vm_id = %vm.id, socket_path, "Spawning Firecracker");

        // Spawn the Firecracker process
        let child = Command::new(&self.firecracker_bin)
            .arg("--socket-path")
            .arg(socket_path)
            .spawn()
            .map_err(|e| {
                VmmError::SpawnFailed(format!("failed to spawn firecracker: {}", e))
            })?;

        let pid = child.id();
        // Drop child to detach; process continues running in background
        drop(child);

        info!(pid, "Firecracker process spawned");

        // Wait for socket to be ready (file to exist)
        if let Err(e) = Self::wait_for_socket(socket_path, Duration::from_secs(5)).await {
            warn!(pid, "Socket not ready, killing process");
            let _ = self.kill(pid).await;
            return Err(e);
        }

        // Configure the VM via HTTP socket API
        if let Err(e) = Self::configure_vm(socket_path, vm).await {
            warn!(pid, "Configuration failed, killing process");
            let _ = self.kill(pid).await;
            return Err(e);
        }

        Ok(SpawnResult { pid })
    }

    async fn kill(&self, pid: u32) -> Result<(), VmmError> {
        info!(pid, "Killing Firecracker process");
        // Use SIGKILL via libc
        unsafe {
            if libc::kill(pid as libc::pid_t, libc::SIGKILL) != 0 {
                return Err(VmmError::KillFailed(
                    format!("kill({}, SIGKILL) failed", pid),
                ));
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_creates_correctly() {
        let vmm = FirecrackerVmm::new("/usr/bin/firecracker".to_string());
        assert_eq!(vmm.firecracker_bin, "/usr/bin/firecracker");
    }

    // Integration tests would require:
    // - firecracker binary available
    // - TAP device pre-created
    // - Kernel and rootfs images available
    // These are marked #[ignore] for MVP
}
```

## src/lib.rs - Updated

```rust
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
```

## Integration with Daemon

In `crates/firebox-api/src/main.rs`:

```rust
// Choose implementation: StubVmm for testing, FirecrackerVmm for real
let vmm = if daemon_cfg.firecracker_bin.contains("stub") {
    Arc::new(firebox_vmm::StubVmm) as Arc<dyn Vmm>
} else {
    Arc::new(firebox_vmm::FirecrackerVmm::new(daemon_cfg.firecracker_bin.clone()))
};

let core = Arc::new(Core::new(
    Arc::new(MemoryStore::new()),
    vmm,
    Arc::new(daemon_cfg),
));
```

Or simpler, always use FirecrackerVmm with a config flag:

```rust
let vmm = Arc::new(FirecrackerVmm::new(daemon_cfg.firecracker_bin.clone()));
let core = Arc::new(Core::new(
    Arc::new(MemoryStore::new()),
    vmm,
    Arc::new(daemon_cfg),
));
```

Tests can continue using `StubVmm`.
