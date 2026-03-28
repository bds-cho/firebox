use firebox_store::NetworkConfig;
use serde::Deserialize;

#[derive(Debug, Clone, Deserialize)]
pub struct DaemonConfig {
    pub firecracker_bin: String,
    pub listen_addr: String,
    pub socket_dir: String,
    pub log_level: String,
}

impl Default for DaemonConfig {
    fn default() -> Self {
        Self {
            firecracker_bin: "/usr/bin/firecracker".to_string(),
            listen_addr: "127.0.0.1:8080".to_string(),
            socket_dir: "/run/firebox/sockets".to_string(),
            log_level: "info".to_string(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct VmConfig {
    pub id: Option<String>,
    pub vcpus: u8,
    pub memory_mb: u32,
    pub kernel: String,
    pub rootfs: String,
    pub network: Option<NetworkConfig>,
}
