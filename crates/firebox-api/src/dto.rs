use firebox_store::{NetworkConfig, Vm};
use serde::{Deserialize, Serialize};

// --- Requests ---

#[derive(Debug, Deserialize)]
pub struct CreateVmRequest {
    pub id: Option<String>,
    pub vcpus: u8,
    pub memory_mb: u32,
    pub kernel: String,
    pub rootfs: String,
    pub network: Option<NetworkConfigDto>,
}

#[derive(Debug, Deserialize)]
pub struct NetworkConfigDto {
    pub tap_device: String,
    pub mac: Option<String>,
}

impl From<NetworkConfigDto> for NetworkConfig {
    fn from(dto: NetworkConfigDto) -> Self {
        NetworkConfig { tap_device: dto.tap_device, mac: dto.mac }
    }
}

// --- Responses ---

#[derive(Debug, Serialize)]
pub struct VmSummary {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct VmDetail {
    pub id: String,
    pub vcpus: u8,
    pub memory_mb: u32,
    pub kernel: String,
    pub rootfs: String,
    pub status: String,
    pub pid: Option<u32>,
}

impl From<Vm> for VmSummary {
    fn from(vm: Vm) -> Self {
        VmSummary { id: vm.id, status: vm.status.to_string() }
    }
}

impl From<Vm> for VmDetail {
    fn from(vm: Vm) -> Self {
        VmDetail {
            id: vm.id,
            vcpus: vm.vcpus,
            memory_mb: vm.memory_mb,
            kernel: vm.kernel,
            rootfs: vm.rootfs,
            status: vm.status.to_string(),
            pid: vm.pid,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct ActionResponse {
    pub status: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    pub error: String,
}
