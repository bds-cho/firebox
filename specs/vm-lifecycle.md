# VM Lifecycle Specification

## Overview

Core owns the VM state machine. Every state transition is explicit, validated, and atomic within the Store.

---

## States

```
Created ──► Running ──► Stopped
                          │
                          ▼
                        Deleted
```

| State     | Description |
|-----------|-------------|
| `Created` | Config validated and stored. No process running. |
| `Running` | Firecracker process is live and VM is booted. |
| `Stopped` | Firecracker process has exited. Config still in Store. |
| `Deleted` | Removed from Store. No further operations possible. |

---

## Transitions

### Created → Running (`start`)

1. Load VM config from Store.
2. VMM::spawn — spawn Firecracker, configure via socket API (kernel, rootfs, vCPUs, memory, TAP device), send InstanceStart.
3. Store::update → `Running`, record Firecracker PID.

**Rejected if:** already `Running`.

---

### Running → Stopped (`stop`)

1. VMM::kill — send SIGKILL to Firecracker process.
2. Firecracker exits; VM is destroyed.
3. Store::update → `Stopped`, clear PID.

**Rejected if:** not `Running`.

---

### Stopped → Running (`start`)

Same as Created → Running. Firecracker is stateless per boot — a fresh process is spawned.

---

### Stopped / Created → Deleted (`delete`)

1. Remove from Store.

**Rejected if:** `Running` (must stop first).

---

## Error Handling

- If VMM::spawn fails, the error is returned immediately. No cleanup needed.
- If the Firecracker process dies unexpectedly, Core detects it on the next `get_vm` call by checking if the PID is still alive. The VM is transitioned to `Stopped` automatically.
- MVP: lazy reconciliation — unexpected death is only detected when the VM is explicitly queried.

---

## Concurrency

- Transitions acquire a write lock on the Store entry for the duration of the operation.
- Concurrent calls on the same VM ID are serialized.
- Calls on different VM IDs are fully independent.
