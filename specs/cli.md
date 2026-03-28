# CLI Specification

## Overview

Command-line interface for managing Firecracker MicroVMs. Communicates with the local firebox daemon via the REST API.

## MVP Scope

Mirrors the API surface exactly — no extra features beyond what the API supports.

---

## Binary Name

```
firebox
```

---

## Global Flags

| Flag | Default | Description |
|------|---------|-------------|
| `--host` | `http://localhost:8080` | Daemon address |

---

## Commands

### `firebox vm create`

Create a new VM.

```
firebox vm create \
  --vcpus <n> \
  --memory <mb> \
  --kernel <path> \
  --rootfs <path> \
  [--id <id>] \
  [--tap <device>] \
  [--mac <address>]
```

Output:
```
Created VM <id>
```

---

### `firebox vm list`

List all VMs.

```
firebox vm list
```

Output:
```
ID                                    STATUS
a1b2c3d4-...                          running
e5f6g7h8-...                          stopped
```

---

### `firebox vm get <id>`

Show details for a single VM.

```
firebox vm get <id>
```

Output:
```
ID:         a1b2c3d4-...
Status:     running
vCPUs:      2
Memory:     256 MB
Kernel:     /path/to/vmlinux
Rootfs:     /path/to/rootfs.ext4
PID:        12345
```

---

### `firebox vm start <id>`

Start a VM.

```
firebox vm start <id>
```

Output:
```
VM <id> started
```

---

### `firebox vm stop <id>`

Stop a VM.

```
firebox vm stop <id>
```

Output:
```
VM <id> stopped
```

---

### `firebox vm delete <id>`

Delete a stopped VM.

```
firebox vm delete <id>
```

Output:
```
VM <id> deleted
```

---

## Error Handling

Errors are printed to stderr with a non-zero exit code:

```
Error: VM not found: <id>
```

Exit codes:
| Code | Meaning |
|------|---------|
| 0    | Success |
| 1    | General error (API error, bad response) |
| 2    | Invalid arguments |

---

## Notes

- MVP: plain text output only. JSON output flag (`--json`) is post-MVP.
- The CLI is a thin HTTP client — no business logic, just translates commands to API calls.
