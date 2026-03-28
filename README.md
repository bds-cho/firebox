# Firebox

A lightweight orchestrator for Firecracker MicroVMs. Written in Rust with an HTTP API and CLI.

Firebox manages the full lifecycle of Firecracker VMs: create, configure, start, stop, and delete — without needing to learn Firecracker's socket API yourself.

## Quick Start

### Build

```bash
cargo build --release
```

Binaries:
- `target/release/firebox-daemon` — HTTP daemon
- `target/release/firebox` — CLI client

### Run Tests

```bash
cargo test
```

All 23 tests pass without requiring Firecracker, kernel, or rootfs files.

### Run the Daemon (with StubVmm - no real VMs)

For quick local testing without Firecracker installed:

```bash
cargo run -p firebox-api
```

Server listens on `http://127.0.0.1:8080`. Requests work but VMs don't actually boot (using `StubVmm`).

Test with the CLI:

```bash
cargo run -p firebox-cli -- vm create \
  --kernel /tmp/vmlinux \
  --rootfs /tmp/rootfs.ext4 \
  --id my-vm

cargo run -p firebox-cli -- vm list
cargo run -p firebox-cli -- vm get my-vm
cargo run -p firebox-cli -- vm start my-vm
cargo run -p firebox-cli -- vm stop my-vm
cargo run -p firebox-cli -- vm delete my-vm
```

---

## Real Firecracker Setup (Ubuntu 20.04+)

To run actual VMs, you need:

1. **Linux with KVM support** (not WSL; native Linux or VM with nested virt)
2. **Firecracker binary**
3. **Linux kernel image** (uncompressed vmlinux)
4. **Rootfs image** (ext4 filesystem)
5. **TAP network device** (optional, for networking)

### Step 1: Install Firecracker

```bash
# Download latest release
wget https://github.com/firecracker-microvm/firecracker/releases/download/v1.7.0/firecracker-v1.7.0-x86_64
chmod +x firecracker-v1.7.0-x86_64
sudo mv firecracker-v1.7.0-x86_64 /usr/local/bin/firecracker

# Verify
firecracker --version
```

**Note:** Firecracker requires KVM. Check:

```bash
grep -c '^processor' /proc/cpuinfo  # Should be > 0
kvm-ok                               # If available, should say "ok"
```

### Step 2: Get a Kernel Image

Option A: Use your distro's kernel:

```bash
# Ubuntu
sudo apt-get install linux-image-generic
cp /boot/vmlinuz-$(uname -r) ./vmlinux.uncompressed

# Decompress if needed (usually already uncompressed)
file ./vmlinux.uncompressed
```

Option B: Build from Linux source (see Firecracker docs).

### Step 3: Create a Rootfs

Option A: Use a pre-built image (Alpine Linux is minimal):

```bash
# Download Alpine minirootfs
wget https://dl-cdn.alpinelinux.org/alpine/latest-stable/releases/x86_64/alpine-minirootfs-3.18.0-x86_64.tar.gz

# Create ext4 image (500 MB)
dd if=/dev/zero of=rootfs.ext4 bs=1M count=500
mkfs.ext4 -F rootfs.ext4
mkdir -p /tmp/mnt
sudo mount rootfs.ext4 /tmp/mnt
sudo tar -xzf alpine-minirootfs-3.18.0-x86_64.tar.gz -C /tmp/mnt
sudo umount /tmp/mnt
```

Option B: Use debootstrap (Debian/Ubuntu):

```bash
# Create minimal Ubuntu rootfs
sudo debootstrap --variant=minbase focal rootfs-dir
# Convert to ext4 image...
```

### Step 4: Create TAP Device (optional, for networking)

```bash
# Create TAP device named tap0
sudo ip tuntap add tap0 mode tap

# Assign IP
sudo ip addr add 192.168.100.1/24 dev tap0
sudo ip link set tap0 up

# Enable IP forwarding if needed
sudo sysctl -w net.ipv4.ip_forward=1
```

### Step 5: Configure Firebox Daemon

Create `/etc/firebox/config.toml`:

```toml
firecracker_bin = "/usr/local/bin/firecracker"
listen_addr     = "127.0.0.1:8080"
socket_dir      = "/tmp/firebox-sockets"
log_level       = "info"
```

Or use environment variables:

```bash
export FIREBOX_FIRECRACKER_BIN=/usr/local/bin/firecracker
export FIREBOX_SOCKET_DIR=/tmp/firebox-sockets
export FIREBOX_LOG_LEVEL=debug
```

### Step 6: Run the Daemon

```bash
mkdir -p /tmp/firebox-sockets
cargo run -p firebox-api --release
```

Output:
```
firebox daemon listening on 127.0.0.1:8080
```

### Step 7: Create and Boot a VM

```bash
# Create VM
cargo run -p firebox-cli -- vm create \
  --id my-vm \
  --kernel /path/to/vmlinux \
  --rootfs /path/to/rootfs.ext4 \
  --tap tap0 \
  --vcpus 2 \
  --memory 256

# List VMs
cargo run -p firebox-cli -- vm list

# Start VM
cargo run -p firebox-cli -- vm start my-vm

# Check status
cargo run -p firebox-cli -- vm get my-vm

# Connect to serial console (if running in another terminal)
# Firecracker doesn't provide a built-in console; use `ps aux | grep firecracker`
# to find the process and confirm it's running

# Stop VM
cargo run -p firebox-cli -- vm stop my-vm

# Delete VM
cargo run -p firebox-cli -- vm delete my-vm
```

---

## Architecture

```
CLI/API → Core → VMM → Firecracker (subprocess)
            ↓
          Store (in-memory)
```

**Components:**

- **Core** — State machine, orchestration, validation
- **VMM** — Process manager (spawns Firecracker, configures via HTTP socket)
- **Store** — VM state storage (in-memory for MVP)
- **API** — Actix-web HTTP daemon
- **CLI** — Pure HTTP client, independent of Core

See `specs/` directory for detailed specifications.

---

## API Reference

### REST Endpoints

All endpoints prefixed with `/api/v1`.

**Create VM:**
```bash
POST /vms
{
  "id": "my-vm",            # optional, generated if omitted
  "vcpus": 2,
  "memory_mb": 256,
  "kernel": "/path/to/vmlinux",
  "rootfs": "/path/to/rootfs.ext4",
  "network": {
    "tap_device": "tap0",
    "mac": "aa:bb:cc:dd:ee:ff"  # optional
  }
}
```

**List VMs:**
```bash
GET /vms
```

**Get VM:**
```bash
GET /vms/{id}
```

**Start VM:**
```bash
POST /vms/{id}/start
```

**Stop VM:**
```bash
POST /vms/{id}/stop
```

**Delete VM:**
```bash
DELETE /vms/{id}
```

### CLI Commands

```bash
firebox vm create --kernel <path> --rootfs <path> [--id <id>] \
  [--vcpus <n>] [--memory <mb>] [--tap <device>] [--mac <addr>]

firebox vm list

firebox vm get <id>

firebox vm start <id>

firebox vm stop <id>

firebox vm delete <id>
```

---

## Configuration

### Daemon Config

Via file `/etc/firebox/config.toml`:

```toml
firecracker_bin = "/usr/local/bin/firecracker"  # path to firecracker binary
listen_addr     = "127.0.0.1:8080"              # HTTP listen address
socket_dir      = "/run/firebox/sockets"        # Unix socket directory
log_level       = "info"                        # trace|debug|info|warn|error
```

Or environment variables (prefix `FIREBOX_`):

```bash
FIREBOX_FIRECRACKER_BIN=/usr/bin/firecracker
FIREBOX_LISTEN_ADDR=0.0.0.0:8080
FIREBOX_SOCKET_DIR=/var/run/firebox
FIREBOX_LOG_LEVEL=debug
```

### VM Config

Specified at creation time via API or CLI. Required:
- `vcpus` — vCPU count (1–8 recommended)
- `memory_mb` — RAM in MB (minimum 128)
- `kernel` — path to uncompressed Linux kernel image
- `rootfs` — path to ext4 root filesystem image

Optional:
- `id` — VM ID (UUIDv4 if not provided)
- `network.tap_device` — TAP device name
- `network.mac` — guest MAC address (random if not provided)

**Boot arguments (hardcoded):**
```
console=ttyS0 reboot=k panic=1 pci=off
```

---

## Development

### Project Structure

```
firebox/
├── Cargo.toml                    # workspace root
├── specs/                        # design specifications
│   ├── architecture.md
│   ├── api.md
│   ├── cli.md
│   ├── vm-lifecycle.md
│   ├── configuration.md
│   └── implementation-plan.md
├── FIRECRACKER_INTEGRATION.md    # Firecracker integration details
├── FIRECRACKER_VMM_CODE.md       # code design for FirecrackerVmm
└── crates/
    ├── firebox-store/            # VM types, Store trait, MemoryStore
    ├── firebox-vmm/              # Vmm trait, StubVmm, FirecrackerVmm
    ├── firebox-core/             # state machine, validation, orchestration
    ├── firebox-api/              # Actix-web HTTP daemon
    └── firebox-cli/              # CLI client
```

### Running Tests

```bash
# All tests
cargo test

# Specific crate
cargo test -p firebox-core

# With logging
RUST_LOG=debug cargo test

# Single test
cargo test test_create_and_get
```

### Code Quality

```bash
# Format
cargo fmt

# Lint
cargo clippy

# Build release
cargo build --release
```

---

## Known Limitations (MVP)

- **Single host only** — no clustering or distributed state
- **In-memory storage** — state lost on daemon restart (no persistence)
- **No snapshots** — can't save/restore VM state
- **No metrics** — no performance data collection
- **No console access** — can't interact with VM serial console
- **Hardcoded boot args** — not customizable per VM
- **TAP devices must pre-exist** — no automatic network setup

---

## Future Enhancements

- Persistent storage (SQLite, PostgreSQL)
- Snapshots and restore
- Multiple drives per VM
- Advanced networking (IP assignment, DNS)
- Metrics and monitoring
- Multi-host orchestration
- VM templates and cloning
- Security (authentication, authorization, isolation)

---

## Troubleshooting

### Firecracker Not Found

```bash
Error: spawn failed: No such file or directory
```

**Fix:**
```bash
# Set correct path
export FIREBOX_FIRECRACKER_BIN=$(which firecracker)
```

### Socket Timeout

```bash
Error: spawn failed: socket not ready after 5s
```

**Causes:**
- Firecracker binary doesn't exist or isn't executable
- Permissions on `socket_dir` (must be writable)
- Kernel doesn't support KVM

**Fix:**
```bash
# Verify Firecracker runs
firecracker --version

# Check socket directory
mkdir -p /tmp/firebox-sockets
chmod 755 /tmp/firebox-sockets
```

### KVM Not Available

```bash
kvm-ok
KVM acceleration can not be used
```

**Solutions:**
- Run on native Linux (not WSL or nested VM)
- Enable nested virtualization in hypervisor (if in a VM)
- Use a cloud instance with KVM support (AWS, GCP, Azure, etc.)

### TAP Device Not Found

```bash
Error: configuration failed: network-interfaces/eth0: HTTP 400
```

**Fix:**
```bash
sudo ip tuntap add tap0 mode tap
```

---

## Contributing

Specs and design docs in `specs/`. Implementation should follow these guides before modifying code.

---

## License

MIT

---

## References

- [Firecracker GitHub](https://github.com/firecracker-microvm/firecracker)
- [Firecracker API Spec](https://firecracker-microvm.github.io/)
- [Firebox Specs](./specs/)
- [Firecracker Integration Design](./FIRECRACKER_INTEGRATION.md)
