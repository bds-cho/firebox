# Configuration Specification

## Overview

Two distinct configuration surfaces: **VM config** (per-VM, provided by the caller) and **Daemon config** (global, set at startup).

---

## VM Config

Defines a single MicroVM. Used by Core, passed in via API or CLI.

```rust
struct VmConfig {
    id: Option<String>,   // generated (UUIDv4) if omitted
    vcpus: u8,            // min: 1
    memory_mb: u32,       // min: 128
    kernel: String,       // absolute path to uncompressed kernel image (vmlinux)
    rootfs: String,       // absolute path to ext4 root filesystem image
    network: Option<NetworkConfig>,
}

struct NetworkConfig {
    tap_device: String,   // name of pre-existing TAP device (e.g. "tap0")
    mac: Option<String>,  // MAC address; randomly generated if omitted
}
```

**Validation rules (enforced by Core, not API):**
- `vcpus` ≥ 1
- `memory_mb` ≥ 128
- `kernel` and `rootfs` paths must exist on disk at create time
- `tap_device` must exist if `network` is provided

---

## Daemon Config

Global settings for the firebox daemon. Loaded once at startup from a config file or environment variables.

```toml
# /etc/firebox/config.toml  (or set via env: FIREBOX_*)

firecracker_bin = "/usr/bin/firecracker"   # path to firecracker binary
listen_addr     = "127.0.0.1:8080"         # API bind address
socket_dir      = "/run/firebox/sockets"   # directory for per-VM FC Unix sockets
log_level       = "info"                   # trace | debug | info | warn | error
```

Environment variable overrides follow the pattern `FIREBOX_<KEY>` (e.g. `FIREBOX_LISTEN_ADDR`).

---

## Firecracker Socket Path Convention

Each VM gets its own Firecracker API socket:

```
{socket_dir}/{vm_id}.sock
```

Core passes this path to VMM at spawn time. VMM creates it; Core uses it for FC API calls during boot configuration.

---

## MVP Constraints

- No hot-reload of daemon config — restart required for changes.
- No per-VM log file configuration — all output goes to the daemon's stdout/stderr.
- Kernel boot args are hardcoded for MVP: `console=ttyS0 reboot=k panic=1 pci=off`.
