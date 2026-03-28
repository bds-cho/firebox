# Firecracker Integration Design

## Overview

Replace `StubVmm` with `FirecrackerVmm`, a real implementation that:
1. Spawns the Firecracker binary with a Unix socket
2. Waits for the socket to be ready (retry loop)
3. Configures the VM via Firecracker's HTTP socket API
4. Sends InstanceStart to boot the VM
5. Returns the Firecracker process PID

## Firecracker HTTP API (via Unix socket)

Firecracker listens on a Unix socket and accepts HTTP requests. All config is sent **before** InstanceStart.

### Config Requests

```
PUT /machine-config
  { "vcpu_count": 2, "mem_size_mib": 128 }

PUT /boot-source
  { "kernel_image_path": "/path/to/vmlinux", "boot_args": "console=ttyS0 reboot=k panic=1 pci=off" }

PUT /drives/{drive_id}
  { "drive_id": "rootfs", "path_on_host": "/path/to/rootfs.ext4", "is_root_device": true, "is_read_only": false }

PUT /network-interfaces/{iface_id}
  { "iface_id": "eth0", "host_dev_name": "tap0", "guest_mac": "aa:bb:cc:dd:ee:ff" }

PUT /actions
  { "action_type": "InstanceStart" }
```

## FirecrackerVmm Implementation

### Location
- Code in `crates/firebox-vmm/src/firecracker.rs`
- Implements the `Vmm` trait
- `StubVmm` remains for testing

### Structure

```rust
pub struct FirecrackerVmm {
    firecracker_bin: String,  // path to firecracker binary
}

impl FirecrackerVmm {
    pub fn new(firecracker_bin: String) -> Self { ... }

    async fn spawn(&self, vm: &Vm, socket_path: &str) -> Result<SpawnResult, VmmError> {
        // 1. Spawn firecracker process
        // 2. Wait for socket to be ready (with timeout)
        // 3. Configure via HTTP socket API
        // 4. Send InstanceStart
        // 5. Return PID
    }

    async fn kill(&self, pid: u32) -> Result<(), VmmError> {
        // SIGKILL via libc
    }
}
```

### Key Subroutines

**1. Spawn Firecracker**
```rust
let child = std::process::Command::new(&self.firecracker_bin)
    .arg("--socket-path")
    .arg(socket_path)
    .spawn()?;

let pid = child.id();
drop(child);  // Detach; process continues running
```

**2. Wait for Socket Ready** (with timeout)
```rust
for attempt in 0..50 {  // ~5 seconds with 100ms sleeps
    if Path::new(socket_path).exists() {
        return Ok(());
    }
    sleep(Duration::from_millis(100)).await;
}
Err(VmmError::SpawnFailed("socket timeout".into()))
```

**3. HTTP Client via Unix Socket**
Use `reqwest` + `hyperlocal` to communicate with Firecracker over Unix socket.
```rust
let client = Client::builder()
    .connector(UnixConnector)
    .build()?;

// Make PUT requests to http+unix://socket-path/endpoint
```

**4. Configure VM** (sequentially)
- MachineConfig (vcpus, memory)
- BootSource (kernel + hardcoded boot args)
- DriveConfig (rootfs)
- NetworkInterface (TAP device + optional MAC)
- InstanceAction (InstanceStart)

Error handling: if any step fails, kill the Firecracker process before returning error.

**5. Kill Process**
```rust
unsafe {
    libc::kill(pid as libc::pid_t, libc::SIGKILL);
}
```

## Dependencies

Add to `crates/firebox-vmm/Cargo.toml`:
```toml
reqwest        = { version = "0.12", features = ["json"] }
hyperlocal     = "0.1"  # Unix socket connector for reqwest
tokio-util     = "0.7"  # tokio helpers if needed
serde_json     = { workspace = true }  # for JSON serialization
```

## Configuration Decisions (MVP)

- **Boot args:** Hardcoded `"console=ttyS0 reboot=k panic=1 pci=off"`
- **Root device:** Always `"rootfs"`, always mounted at drive ID `"rootfs"`
- **Network:** Single interface `"eth0"` on TAP device from VM config
- **MAC address:** Taken from VM config if provided, else Firecracker auto-generates
- **Socket timeout:** 5 seconds
- **No cleanup:** Socket files left behind (can be cleaned manually or at daemon startup)

## DaemonConfig Usage

The `firecracker_bin` path is configured via `DaemonConfig`:
```toml
firecracker_bin = "/usr/bin/firecracker"  # or /path/to/firecracker
```

At daemon startup, pass this to FirecrackerVmm:
```rust
let vmm = Arc::new(FirecrackerVmm::new(daemon_cfg.firecracker_bin.clone()));
```

## Testing Strategy

1. **Unit tests** on FirecrackerVmm (mock the socket/HTTP calls if needed)
2. **Integration tests** require:
   - Firecracker binary available at a known path
   - TAP device pre-created
   - Kernel and rootfs images available
3. **For MVP:** Skip integration tests or mark them `#[ignore]` and run manually

## Error Cases

- Firecracker binary not found → `VmmError::SpawnFailed`
- Socket timeout → `VmmError::SpawnFailed("socket timeout")`
- HTTP configuration fails → `VmmError::SpawnFailed` with details
- Kill fails → `VmmError::KillFailed`

## Future Enhancements (post-MVP)

- Parse Firecracker responses (currently ignore)
- Detect if configuration fails and provide better error messages
- Snapshot/restore via Firecracker API
- Rate limiting, metrics collection
- Automatic socket cleanup at daemon shutdown
- Support for additional drives (secondary disks)
