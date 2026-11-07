# mu-cd-ipc

<p align="center">
  <a href="https://hook.lma1980.duckdns.org/jobs">
    <img src="https://hook.lma1980.duckdns.org/cards/LMA1980/mu-cd-ipc/card.svg" alt="mu-cd-ipc Factory Scorecard" width="520"/>
  </a>
</p>

<p align="center">
  <a href="https://hook.lma1980.duckdns.org/jobs"><img src="https://hook.lma1980.duckdns.org/badges/LMA1980/mu-cd-ipc/status.svg" alt="CI Status"/></a>
  <a href="https://hook.lma1980.duckdns.org/jobs"><img src="https://hook.lma1980.duckdns.org/badges/LMA1980/mu-cd-ipc/score.svg" alt="Quality Score"/></a>
  <a href="https://hook.lma1980.duckdns.org/jobs"><img src="https://hook.lma1980.duckdns.org/badges/LMA1980/mu-cd-ipc/coverage.svg" alt="Coverage"/></a>
  <a href="https://hook.lma1980.duckdns.org/jobs"><img src="https://hook.lma1980.duckdns.org/badges/LMA1980/mu-cd-ipc/benchmark.svg" alt="Benchmark"/></a>
</p>

---

High-performance, zero-copy inter-process communication (IPC) and distributed RPC protocol crate connecting `MuConDe-controller` and `MuConDe-agent`.

---

## Protocol Framing

`mu-cd-ipc` uses an aligned 24-byte binary framing header (`IpcHeader`) compatible with `zerocopy::FromBytes` and `zerocopy::IntoBytes`.

```text
+-----------------------+----------------------------------------------------+
| IpcHeader (24 Bytes)  | Payload (Variable Length)                          |
+-----------------------+----------------------------------------------------+
| Magic: b"MCDE" (4B)   | If MsgType == 0x01: LeaseAcquireRequest / Response |
| Version: u16          | If MsgType == 0x02: StageDeploymentRequest / Resp  |
| MsgType: u16          | If MsgType == 0x03: HealthProbeTelemetry           |
| PayloadLen: u32       | If MsgType == 0x04: PromoteSlotRequest / Response  |
| Flags: u32            | If MsgType == 0x05: RollbackSlotRequest / Response |
| Sequence: u64         | If MsgType == 0x06: HeartbeatTelemetry             |
+-----------------------+----------------------------------------------------+
```

### Wire Header Layout

```rust
use zerocopy::{FromBytes, IntoBytes, KnownLayout, Immutable};

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C, align(8))]
pub struct IpcHeader {
    pub magic: [u8; 4],     // Canonical identifier: b"MCDE"
    pub version: u16,       // Wire protocol version (e.g. 1)
    pub msg_type: u16,      // Message type discriminant
    pub payload_len: u32,   // Payload length in bytes (excludes 24B header)
    pub flags: u32,         // Bitwise flags (compression, encryption, urgent)
    pub sequence: u64,      // Monotonically increasing sequence number
}
```

---

## Message Contracts Catalog

Every frame carries a `msg_type` discriminant in the header; the payload is the matching
fory-encoded struct registered under the listed fory ID (see `create_fory_runtime`).

| Message | `msg_type` | fory ID | Direction | Description |
|---|---|---|---|---|
| `DeploymentLeaseRequest` | `0x01` | 1 | Controller ➔ Coordinator | Requests an exclusive TTL deployment lease on a target node |
| `DeploymentLeaseGrant` | `0x02` | 2 | Coordinator ➔ Controller | Grants or refuses the lease with `lease_id`, slots and expiry |
| `DeploymentLeaseRevoke` | `0x03` | 3 | Controller ➔ Agent | Revokes a lease (pre-emption or TTL expiry) |
| `StageDeploymentRequest` | `0x04` | 4 | Controller ➔ Agent | Stages the candidate artifact into the inactive slot |
| `StageDeploymentResponse` | `0x05` | 5 | Agent ➔ Controller | Acknowledges staging and the subvolume path |
| `SlotSwapCommand` | `0x06` | 6 | Controller ➔ Agent | Atomic slot swap to the candidate slot, then warmup |
| `RollbackCommand` | `0x07` | 7 | Controller ➔ Agent | Restores `restore_slot` immediately |
| `IntegrationTestReport` | `0x08` | 8 | Agent ➔ Controller | **Final** aggregate gate verdict |
| `ArtifactTaintNotification` | `0x09` | 9 | Agent ➔ Controller | Taints a failed artifact digest |
| `HealthProbeTelemetry` | `0x0A` | 10 | Agent ➔ Controller | Per-tick health probe telemetry |
| `HeartbeatTelemetry` | `0x0B` | 11 | Agent ⇄ Controller | Node liveness and free space |
| `DeploymentSlot` (enum) | — | 12 | — | `SlotA` / `SlotB` |
| `IntegrationTestProgress` | `0x0C` | 13 | Agent ➔ Controller | **0.2.0.** Per-test gate progress (`Pending`/`Running`/`Passed`/`Failed`, `critical`, `duration_ms`) |
| `TestState` (enum) | — | 14 | — | **0.2.0.** State of a single integration test |

### Registry rules (additive only)

- fory IDs `1..=12` and discriminants `0x01..=0x0B` are **frozen**. New messages are appended with
  new IDs; existing IDs are never renumbered or reused.
- Receivers dispatch on `msg_type` **before** decoding the payload and skip frames they do not
  recognise. A 0.1.x controller therefore ignores `0x0C` progress frames and still decodes the
  final `IntegrationTestReport`; see `tests/it_progress_compat_tests.rs`. Use
  `is_legacy_msg_type()` / `is_known_msg_type()` to implement the same rule.
- `IntegrationTestReport` stays the final verdict. `IntegrationTestProgress` is advisory live state
  for UIs and the controller's gate tracker.

### Downstream dependency update (agent and controller)

`MuConDe-agent` and `MuConDe-controller` depend on this crate by git branch. To consume a new
protocol version:

1. While the ipc change is still an open PR, a downstream feature branch may temporarily point at
   the ipc feature branch: `mu-cd-ipc = { git = "https://github.com/LMA1980/mu-cd-ipc.git", branch = "feat/<name>" }`.
2. After the ipc PR merges, switch the dependency back to `branch = "main"` and run
   `cargo update -p mu-cd-ipc` so `Cargo.lock` pins the merged commit.
3. Deploy in order: controller first (it tolerates both old and new agents), then agents. Old
   controllers simply skip the new frames, so either order is safe for 0.2.0.

---

## Type Invariants

1. **`usize` vs `u32` Discipline**:
   - `usize` is reserved strictly for local in-memory collections, array indices, and buffer pointer offsets.
   - `u32` is strictly used for wire packet lengths, sequence flags, message types, and network IDs.
2. **Zero-Copy Slicing**:
   - Message payloads are parsed zero-copy from byte buffers; unaligned memory access is forbidden.
3. **Little-Endian Wire Canonicalization**:
   - All multi-byte integer fields are encoded in standard Little-Endian format (`u16`, `u32`, `u64`).

---

## License

Licensed under the GNU Lesser General Public License v3.0 with the LGPL-3.0 Linking Exception (LGPL-3.0-only WITH LGPL-3.0-linking-exception). See LICENSE for details.
