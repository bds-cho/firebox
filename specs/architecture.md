# Architecture Specification

## Design Principles

- **Speed first** — minimize latency on every operation, especially VM boot. No unnecessary layers.
- **Modularity** — each component has a single responsibility and a clean interface. Components are swappable.
- **Core as the source of truth** — all business logic lives in core. API and CLI are thin consumers.

---

## Component Overview

```
┌─────────┐     ┌─────────┐
│   CLI   │     │   API   │  (Actix-web)
└────┬────┘     └────┬────┘
     │               │
     └──────┬────────┘
            │
       ┌────▼────┐
       │  Core   │  ← single source of truth: state machine + process management
       └────┬────┘
            │
    ┌───────┴──────────┐
    │                  │
┌───▼──┐         ┌─────▼──────┐
│ VMM  │         │   Store    │
└───┬──┘         └────────────┘
    │
┌───▼────────────────────────────┐
│  Firecracker                   │
│  (self-contained hypervisor,   │
│   manages VM, network, storage)│
└────────────────────────────────┘
```

---

## Components

### Core

The central orchestrator. All operations flow through it.

- Owns VM state machine (created → running → stopped → deleted)
- Orchestrates VMM to spawn and kill Firecracker processes
- Delegates state persistence to Store
- Validates VM config before passing to Firecracker
- Exposes a clean async Rust API consumed by both CLI and API handler
- No HTTP, no serialization — pure logic

### API

Actix-web HTTP server. Thin adapter layer only.

- Deserializes requests → calls Core
- Serializes Core responses → HTTP responses
- No business logic
- Runs as a long-lived daemon process

### CLI

Binary that makes HTTP calls to the API daemon.

- Thin HTTP client
- Formats responses for terminal output
- No direct dependency on Core — goes through the API

### VMM

Manages the Firecracker subprocess lifecycle per VM.

- Spawns `firecracker` binary with VM config (kernel, rootfs, memory, vCPUs, TAP device, etc.)
- Passes config to Firecracker via its HTTP socket API before boot
- Firecracker handles all VM internals: boot, network, storage, lifecycle
- Core tracks the Firecracker process PID
- Handles stop via SIGKILL
- One Firecracker process per VM

### Store

VM state storage.

- For MVP: in-memory (`HashMap` behind an `Arc<RwLock>`)
- Interface designed for a future persistent backend (SQLite or flat file) without changing callers

---

## Data Flow — VM Create + Start

```
CLI/API
  → Core::create_vm(config)
      → validate config (kernel/rootfs exist, memory ≥ 128MB, etc.)
      → Store::insert(vm)
      → return VM

  → Core::start_vm(id)
      → Store::get(id)
      → VMM::spawn(vm)
          → spawn firecracker process
          → configure via FC socket API (boot source, drives, network interfaces)
          → send InstanceStart action
          → Firecracker boots the VM and manages it
      → Store::update(id, status=running, pid=N)
      → return VM
```

---

## Concurrency Model

- Core holds a single `Arc<RwLock<Store>>` — reads are concurrent, writes are exclusive.
- Each VMM runs its Firecracker process independently.
- Actix-web handles HTTP concurrency via its async runtime (Tokio).
- No global locks on the hot path beyond Store access.

---

## MVP Constraints

- Single host only — no distributed state.
- Store is in-memory; state is lost on daemon restart.
- TAP devices are pre-created by the operator; we just reference them by name.
- One Firecracker binary path, configured at daemon startup.
- Firecracker handles all VM internals; we just orchestrate the process.
