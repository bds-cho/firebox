use std::path::Path;
use std::process::{Command, Stdio};
use std::time::Duration;

use async_trait::async_trait;
use firebox_store::Vm;
use http::Request;
use hyper::body::Body;
use hyper::Client;
use hyperlocal::{UnixConnector, Uri as UnixUri};
use serde::Serialize;
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
        let connector = UnixConnector;
        let client: Client<UnixConnector> = Client::builder().build(connector);

        // 1. Machine config
        let machine_cfg = MachineConfig {
            vcpu_count: vm.vcpus,
            mem_size_mib: vm.memory_mb,
        };
        info!("Configuring machine with {} vCPUs, {} MB memory", vm.vcpus, vm.memory_mb);
        Self::send_config(&client, socket_path, "/machine-config", machine_cfg).await?;

        // 2. Boot source (kernel + hardcoded boot args)
        let boot_cfg = BootSource {
            kernel_image_path: vm.kernel.clone(),
            boot_args: "console=ttyS0 reboot=k panic=1 pci=off".to_string(),
        };
        info!("Configuring boot source: {}", vm.kernel);
        Self::send_config(&client, socket_path, "/boot-source", boot_cfg).await?;

        // 3. Root drive (rootfs)
        let drive_cfg = DriveConfig {
            drive_id: "rootfs".to_string(),
            path_on_host: vm.rootfs.clone(),
            is_root_device: true,
            is_read_only: false,
        };
        info!("Configuring root drive: {}", vm.rootfs);
        Self::send_config(&client, socket_path, "/drives/rootfs", drive_cfg).await?;

        // 4. Network interface (if configured)
        if let Some(ref net_cfg) = vm.network {
            let net_iface = NetworkInterface {
                iface_id: "eth0".to_string(),
                host_dev_name: net_cfg.tap_device.clone(),
                guest_mac: net_cfg.mac.clone(),
            };
            info!("Configuring network interface on TAP device: {}", net_cfg.tap_device);
            Self::send_config(&client, socket_path, "/network-interfaces/eth0", net_iface).await?;
        }

        // 5. Start the VM
        let action = InstanceAction {
            action_type: "InstanceStart".to_string(),
        };
        info!("Starting VM instance");
        Self::send_config(&client, socket_path, "/actions", action).await?;

        info!("VM configured and started successfully");
        Ok(())
    }

    /// Helper to send a PUT request with JSON body to Firecracker
    async fn send_config<T: Serialize>(
        client: &Client<UnixConnector>,
        socket_path: &str,
        endpoint: &str,
        body: T,
    ) -> Result<(), VmmError> {
        let json_body = serde_json::to_string(&body)
            .map_err(|e| VmmError::SpawnFailed(format!("json serialization failed: {}", e)))?;

        let uri: http::Uri = UnixUri::new(socket_path, endpoint).into();

        let req = Request::put(uri)
            .header("Content-Type", "application/json")
            .body(Body::from(json_body))
            .map_err(|e| VmmError::SpawnFailed(format!("request build failed: {}", e)))?;

        let resp = client
            .request(req)
            .await
            .map_err(|e| {
                VmmError::SpawnFailed(format!("{} failed: {}", endpoint, e))
            })?;

        if !resp.status().is_success() {
            return Err(VmmError::SpawnFailed(format!(
                "{}: HTTP {}",
                endpoint,
                resp.status()
            )));
        }

        Ok(())
    }
}

#[async_trait]
impl Vmm for FirecrackerVmm {
    async fn spawn(&self, vm: &Vm, socket_path: &str) -> Result<SpawnResult, VmmError> {
        info!(vm_id = %vm.id, socket_path, "Spawning Firecracker");

        // Spawn the Firecracker process
        // Redirect stdout/stderr to null to prevent VM console appearing in daemon logs
        let child = Command::new(&self.firecracker_bin)
            .arg("--api-sock")
            .arg(socket_path)
            .stdout(Stdio::null())
            .stderr(Stdio::null())
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
}
