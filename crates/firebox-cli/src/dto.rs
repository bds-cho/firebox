use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize)]
pub struct CreateVmRequest {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub vcpus: u8,
    pub memory_mb: u32,
    pub kernel: String,
    pub rootfs: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub network: Option<NetworkConfigDto>,
}

#[derive(Debug, Serialize)]
pub struct NetworkConfigDto {
    pub tap_device: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub mac: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct VmSummary {
    pub id: String,
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct VmDetail {
    pub id: String,
    pub vcpus: u8,
    pub memory_mb: u32,
    pub kernel: String,
    pub rootfs: String,
    pub status: String,
    pub pid: Option<u32>,
}

#[derive(Debug, Deserialize)]
pub struct ActionResponse {
    pub status: String,
}

#[derive(Debug, Deserialize)]
pub struct ErrorResponse {
    pub error: String,
}
