# Implementation Plan

## Workspace and Crate Structure

```
firebox/
├── Cargo.toml                    # workspace root
├── crates/
│   ├── firebox-store/            # lib — VM types, Store trait, MemoryStore
│   ├── firebox-vmm/              # lib — Vmm trait + StubVmm (Firecracker boundary)
│   ├── firebox-core/             # lib — orchestration, state machine, validation
│   ├── firebox-api/              # bin — Actix-web daemon
│   └── firebox-cli/              # bin — CLI client
└── specs/
```

### Workspace `Cargo.toml`

```toml
[workspace]
resolver = "2"
members = [
    "crates/firebox-store",
    "crates/firebox-vmm",
    "crates/firebox-core",
    "crates/firebox-api",
    "crates/firebox-cli",
]

[workspace.dependencies]
tokio       = { version = "1", features = ["full"] }
serde       = { version = "1", features = ["derive"] }
serde_json  = "1"
uuid        = { version = "1", features = ["v4"] }
thiserror   = "1"
tracing     = "1"
async-trait = "0.1"
```

---

## Crate Designs

### `firebox-store`

Canonical VM types and storage abstraction. Every other crate depends on this — built first.

**Key types:**
```rust
pub enum VmStatus { Created, Running, Stopped }

pub struct Vm {
    pub id: String,
    pub vcpus: u8,
    pub memory_mb: u32,
    pub kernel: String,
    pub rootfs: String,
    pub network: Option<NetworkConfig>,
    pub status: VmStatus,
    pub pid: Option<u32>,
}

pub struct NetworkConfig {
    pub tap_device: String,
    pub mac: Option<String>,
}
```

**Store trait:**
```rust
#[async_trait]
pub trait Store: Send + Sync + 'static {
    async fn insert(&self, vm: Vm) -> Result<(), StoreError>;
    async fn get(&self, id: &str) -> Result<Option<Vm>, StoreError>;
    async fn list(&self) -> Result<Vec<Vm>, StoreError>;
    async fn update(&self, vm: Vm) -> Result<(), StoreError>;
    async fn remove(&self, id: &str) -> Result<(), StoreError>;
}
```

**`MemoryStore`:** `Arc<tokio::sync::RwLock<HashMap<String, Vm>>>` — concurrent reads, exclusive writes.

**File layout:**
```
src/
  lib.rs      (Vm, VmStatus, NetworkConfig, StoreError re-exports)
  store.rs    (Store trait)
  memory.rs   (MemoryStore)
```

---

### `firebox-vmm`

The primary architectural boundary deferring real Firecracker integration. Core programs against the trait — the real driver will be a third implementation added later without changing anything else.

**Vmm trait:**
```rust
pub struct SpawnResult { pub pid: u32 }

#[async_trait]
pub trait Vmm: Send + Sync + 'static {
    /// Spawn a VM process. socket_path is the FC Unix socket path Core computed.
    async fn spawn(&self, vm: &Vm, socket_path: &str) -> Result<SpawnResult, VmmError>;
    /// Forcefully stop a running VM.
    async fn kill(&self, pid: u32) -> Result<(), VmmError>;
}
```

**`StubVmm`:** Returns `pid: 99999` from `spawn` (a fake but plausible PID that will fail `kill(pid, 0)` checks, simulating process death for the lazy reconciliation path).

**File layout:**
```
src/
  lib.rs    (Vmm trait, SpawnResult, VmmError)
  stub.rs   (StubVmm)
```

---

### `firebox-core`

The heart of the system. All business logic, state machine enforcement, and orchestration.

**Design choice — generics over dynamic dispatch:**
```rust
pub struct Core<S: Store, V: Vmm> {
    store: Arc<S>,
    vmm:   Arc<V>,
    config: Arc<DaemonConfig>,
}
```
Generics allow monomorphization (zero-cost dispatch), consistent with "speed first." The binary has exactly one concrete instantiation.

**Public API:**
```rust
impl<S: Store, V: Vmm> Core<S, V> {
    pub fn new(store: Arc<S>, vmm: Arc<V>, config: Arc<DaemonConfig>) -> Self;
    pub async fn create_vm(&self, config: VmConfig) -> Result<Vm, CoreError>;
    pub async fn get_vm(&self, id: &str) -> Result<Vm, CoreError>;
    pub async fn list_vms(&self) -> Result<Vec<Vm>, CoreError>;
    pub async fn start_vm(&self, id: &str) -> Result<Vm, CoreError>;
    pub async fn stop_vm(&self, id: &str) -> Result<Vm, CoreError>;
    pub async fn delete_vm(&self, id: &str) -> Result<(), CoreError>;
}
```

**Socket path convention** (Core computes, VMM receives):
```rust
fn socket_path(&self, vm_id: &str) -> String {
    format!("{}/{}.sock", self.config.socket_dir, vm_id)
}
```

**Lazy PID reconciliation in `get_vm`:**
```rust
// if status is Running but PID is dead, transition to Stopped
if vm.status == VmStatus::Running {
    if let Some(pid) = vm.pid {
        if unsafe { libc::kill(pid as i32, 0) } != 0 {
            vm.status = VmStatus::Stopped;
            vm.pid = None;
            self.store.update(vm.clone()).await?;
        }
    }
}
```

**`CoreError`:**
```rust
pub enum CoreError {
    NotFound(String),
    Conflict(String),     // invalid state transition
    Validation(String),   // bad config
    Vmm(#[from] VmmError),
    Store(#[from] StoreError),
    Internal(String),
}
```

**File layout:**
```
src/
  lib.rs      (re-exports)
  config.rs   (DaemonConfig, VmConfig)
  error.rs    (CoreError)
  core.rs     (Core<S,V,N> + all methods)
```

---

### `firebox-api`

Thin Actix-web daemon. Deserializes → Core → serializes. No business logic.

**Route table:**
| Method   | Path                       | Handler     |
|----------|----------------------------|-------------|
| POST     | `/api/v1/vms`              | create_vm   |
| GET      | `/api/v1/vms`              | list_vms    |
| GET      | `/api/v1/vms/{id}`         | get_vm      |
| DELETE   | `/api/v1/vms/{id}`         | delete_vm   |
| POST     | `/api/v1/vms/{id}/start`   | start_vm    |
| POST     | `/api/v1/vms/{id}/stop`    | stop_vm     |

**`CoreError` → HTTP:**
```rust
impl ResponseError for CoreError {
    fn status_code(&self) -> StatusCode {
        match self {
            CoreError::NotFound(_)   => StatusCode::NOT_FOUND,            // 404
            CoreError::Conflict(_)   => StatusCode::CONFLICT,             // 409
            CoreError::Validation(_) => StatusCode::BAD_REQUEST,          // 400
            _                        => StatusCode::INTERNAL_SERVER_ERROR, // 500
        }
    }
}
```

**Config loading:** `figment` or `config` crate — layers `config.toml` then `FIREBOX_*` env vars.

**File layout:**
```
src/
  main.rs         (startup, HttpServer)
  config.rs       (parse DaemonConfig from TOML + env)
  error.rs        (ResponseError for CoreError)
  dto.rs          (request/response serde structs)
  handlers/
    mod.rs
    vms.rs        (all six handlers)
```

---

### `firebox-cli`

Pure HTTP client. Zero dependency on firebox crates.

**Command structure:**
```
firebox [--host <url>] vm <create|list|get|start|stop|delete> [args]
```

**Exit codes:** 0 = success, 1 = API/runtime error, 2 = invalid args.

**HTTP client:** `reqwest` async with `json` feature.

**File layout:**
```
src/
  main.rs     (Cli, Commands, entry point, exit code enforcement)
  client.rs   (reqwest helpers)
  dto.rs      (mirror of API response types)
  format.rs   (terminal output formatting)
```

---

## Implementation Order

### Phase 1 — Foundations
1. Workspace `Cargo.toml`
2. `firebox-store` — types, trait, `MemoryStore`, unit tests
3. `firebox-vmm` — trait, `StubVmm`, unit tests

**Gate:** `cargo test -p firebox-store -p firebox-vmm` passes.

### Phase 2 — Core
4. `firebox-core` config and error types
5. `create_vm`, `get_vm`, `list_vms`
6. `start_vm`, `stop_vm` (via StubVmm)
7. `delete_vm`
8. Lazy PID reconciliation in `get_vm`
9. Integration tests for full lifecycle

**Gate:** `cargo test -p firebox-core` passes, all state transitions covered.

### Phase 3 — API daemon
10. Scaffold Actix-web binary
11. Config loading
12. All six route handlers
13. `ResponseError` for `CoreError`
14. Integration tests via `actix-web::test`

**Gate:** `cargo test -p firebox-api` passes; `curl` smoke test works against running daemon.

### Phase 4 — CLI
15. Scaffold with `clap` (derive)
16. All six commands
17. Output formatting and error/exit code handling

**Gate:** Manual smoke test: create → start → get → stop → delete against live daemon.

### Phase 5 — Polish
18. `tracing-subscriber` init in daemon
19. `cargo clippy` + `cargo fmt --check` CI gate

**Gate:** All tests pass, code is formatted and lint-clean.

### Phase 6 — Firecracker Integration (Post-MVP)

20. Add dependencies to `firebox-vmm`: `reqwest`, `hyperlocal`
21. Implement `FirecrackerVmm` struct in `src/firecracker.rs`:
    - `spawn()` — spawn Firecracker, wait for socket, configure VM, return PID
    - `kill()` — SIGKILL via libc
    - Helper functions for socket wait and HTTP configuration
22. Configure VM via Firecracker HTTP socket API:
    - MachineConfig (vcpus, memory)
    - BootSource (kernel + boot args)
    - DriveConfig (rootfs)
    - NetworkInterface (TAP device + MAC)
    - InstanceAction (InstanceStart)
23. Error handling: if any config step fails, kill the process before returning error
24. Update daemon to use `FirecrackerVmm::new()` instead of `StubVmm`
25. Add `#[ignore]` integration tests (require real Firecracker binary + TAP device)
26. Update docs with Firecracker setup instructions

**Gate:** Daemon runs against real Firecracker (requires manual setup: firecracker binary, TAP device, kernel/rootfs images).

---

## Key Design Decisions

| Decision | Rationale |
|----------|-----------|
| Generics in `Core<S,V>` over `dyn Trait` | Monomorphization = zero-cost dispatch. Single concrete instantiation in practice. |
| `StubVmm` returns PID `99999` | Realistic simulation — fake PID will fail `kill(pid, 0)`, exercising the lazy reconciliation path. |
| VMM is a process manager only | Firecracker handles all VM internals (network, storage, boot). VMM spawns and kills the Firecracker process. |
| Core computes FC socket path, passes to VMM | Core owns naming conventions; VMM is a narrow process manager. |
| Config is `Arc<DaemonConfig>`, read-only after startup | Cheap to share across handlers; no locking needed. |
| CLI has no dependency on firebox crates | Clean separation; CLI is deployable independently against any compatible daemon. |

---

## Firecracker HTTP Socket API Summary

All configuration is sent via PUT requests to a Unix socket before `InstanceStart`.

**Socket URI format:** `http+unix://socket-path/endpoint`

**Endpoints:**
- `PUT /machine-config` — vCPU count and memory size
- `PUT /boot-source` — kernel image path and boot arguments
- `PUT /drives/{drive_id}` — block device (e.g., rootfs)
- `PUT /network-interfaces/{iface_id}` — network interface (TAP device, MAC)
- `PUT /actions` — sends `{"action_type": "InstanceStart"}` to boot the VM

**Key MVP choices:**
- Boot args are hardcoded: `"console=ttyS0 reboot=k panic=1 pci=off"`
- Root drive is always ID `"rootfs"`, mounted as root device
- Network interface is always ID `"eth0"`
- Socket timeout is 5 seconds
- If any config step fails, process is killed before returning error

See `FIRECRACKER_INTEGRATION.md` and `FIRECRACKER_VMM_CODE.md` for detailed design and implementation.

---

## Future Enhancements (Beyond MVP)

- **Snapshots:** Save/restore VM state via Firecracker snapshot API
- **Secondary drives:** Support additional block devices
- **Advanced networking:** Multiple interfaces, custom MTU, rate limiting
- **Metrics:** Collect Firecracker performance data
- **Error recovery:** Detect Firecracker crashes and transition VMs to stopped state
- **Daemonization:** Fork to background, proper signal handling
- **Persistent state:** Store VM configs to disk, recover on restart
- **Multi-tenant:** Isolation, per-user quotas, resource limits
