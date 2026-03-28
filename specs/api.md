# API Specification

## Overview

REST API built with Actix-web. Serves as the primary interface for managing Firecracker MicroVMs.

## MVP Scope

The MVP API covers the minimum needed to create, inspect, and control VMs. No auth, no multi-tenancy, no clustering — single-host only.

**In scope:**
- VM CRUD
- VM power operations (start, stop)
- Basic VM status

**Out of scope (post-MVP):**
- Authentication / authorization
- Snapshots / restore
- Live migration
- Metrics / stats endpoints
- Websocket console access

---

## Base URL

```
http://localhost:8080/api/v1
```

---

## Resources

### VMs

#### Create VM
`POST /vms`

Request body:
```json
{
  "id": "string (optional, generated if omitted)",
  "vcpus": 1,
  "memory_mb": 128,
  "kernel": "/path/to/vmlinux",
  "rootfs": "/path/to/rootfs.ext4",
  "network": {
    "tap_device": "tap0",
    "mac": "AA:BB:CC:DD:EE:FF (optional)"
  }
}
```

Response `201 Created`:
```json
{
  "id": "string",
  "status": "created"
}
```

---

#### List VMs
`GET /vms`

Response `200 OK`:
```json
[
  {
    "id": "string",
    "status": "created | running | stopped"
  }
]
```

---

#### Get VM
`GET /vms/{id}`

Response `200 OK`:
```json
{
  "id": "string",
  "vcpus": 1,
  "memory_mb": 128,
  "kernel": "/path/to/vmlinux",
  "rootfs": "/path/to/rootfs.ext4",
  "status": "created | running | stopped",
  "pid": 12345
}
```

---

#### Delete VM
`DELETE /vms/{id}`

- VM must be stopped before deletion.

Response `204 No Content`

---

### VM Actions

#### Start VM
`POST /vms/{id}/start`

- Spawns the Firecracker process and boots the VM.

Response `200 OK`:
```json
{ "status": "running" }
```

---

#### Stop VM
`POST /vms/{id}/stop`

- Sends shutdown signal to the Firecracker process.
- For MVP: forceful kill (SIGKILL) is acceptable.

Response `200 OK`:
```json
{ "status": "stopped" }
```

---

## Error Format

All errors return a consistent body:
```json
{
  "error": "human-readable message"
}
```

Common status codes:
| Code | Meaning |
|------|---------|
| 400  | Bad request / invalid config |
| 404  | VM not found |
| 409  | Conflict (e.g. start a running VM) |
| 500  | Internal server error |

---

## Notes

- VM state is stored in-memory for MVP (no persistence across daemon restarts).
- IDs are UUIDs v4.
- The API does not proxy to the Firecracker API socket directly — it manages Firecracker processes and abstracts over them.
