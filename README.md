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

______________________________________________________________________

Zero-copy inter-process communication (IPC) and distributed RPC protocol crate in the [`mu-cd`](https://github.com/LMA1980/mu-cd) workspace connecting [`mu-cd-controller`](https://github.com/LMA1980/mu-cd-controller) and [`mu-cd-agent`](https://github.com/LMA1980/mu-cd-agent).

______________________________________________________________________

## Protocol Framing (`src/framing.rs`)

`mu-cd-ipc` uses an aligned 24-byte binary framing header (`IpcHeader`, magic `b"MCDE"`, version `1`, max frame `16 MiB`) compatible with `zerocopy::FromBytes` and `zerocopy::IntoBytes`.

```text
+-----------------------+----------------------------------------------------+
| IpcHeader (24 Bytes)  | Payload (Variable Length)                          |
+-----------------------+----------------------------------------------------+
| Magic: b"MCDE" (4B)   | 0x01: DeploymentLeaseRequest                       |
| Version: u16 (1)      | 0x02: DeploymentLeaseGrant                         |
| MsgType: u16          | 0x03: DeploymentLeaseRevoke                        |
| PayloadLen: u32       | 0x04: StageDeploymentRequest                       |
| Flags: u32            | 0x05: StageDeploymentResponse                      |
| Sequence: u64         | 0x06: SlotSwapCommand                              |
|                       | 0x07: RollbackCommand                              |
|                       | 0x08: IntegrationTestReport                        |
|                       | 0x09: ArtifactTaintNotification                    |
|                       | 0x0A: HealthProbeTelemetry                         |
|                       | 0x0B: HeartbeatTelemetry                           |
|                       | 0x0C: IntegrationTestProgress (0.2.0 additive)     |
|                       | 0x0D: ScenarioExecutionPlan (0.3.0 additive)       |
|                       | 0x0E: ScenarioPlanAck (0.3.0 additive)             |
+-----------------------+----------------------------------------------------+
```

### Wire Header Layout

```rust
use zerocopy::{FromBytes, IntoBytes, KnownLayout, Immutable};

#[derive(Clone, Copy, Debug, PartialEq, Eq, FromBytes, IntoBytes, KnownLayout, Immutable)]
#[repr(C, align(8))]
pub struct IpcHeader {
    pub magic: [u8; 4],     // Canonical identifier: b"MCDE"
    pub version: u16,       // Wire protocol version (1)
    pub msg_type: u16,      // Message type discriminant (0x01..=0x0E)
    pub payload_len: u32,   // Payload length in bytes (excludes 24B header)
    pub flags: u32,         // FLAG_COMPRESSED (0x01), FLAG_END_OF_STREAM (0x02)
    pub sequence: u64,      // Monotonically increasing sequence number
}
```

______________________________________________________________________

## Message Contracts Catalog

Every frame carries a `msg_type` discriminant in the header; the payload is the matching Fory-encoded struct registered under the listed Fory ID (see `create_fory_runtime`).

| Message                     | `msg_type` | Constant                     | Fory ID | Direction                | Description                                                                                          |
| --------------------------- | ---------- | ---------------------------- | ------- | ------------------------ | ---------------------------------------------------------------------------------------------------- |
| `DeploymentLeaseRequest`    | `0x01`     | `MSG_TYPE_LEASE_REQ`         | 1       | Controller ➔ Coordinator | Requests an exclusive TTL deployment lease on a target node                                          |
| `DeploymentLeaseGrant`      | `0x02`     | `MSG_TYPE_LEASE_GRANT`       | 2       | Coordinator ➔ Controller | Grants or refuses the lease with `lease_id`, slots and expiry                                        |
| `DeploymentLeaseRevoke`     | `0x03`     | `MSG_TYPE_LEASE_REVOKE`      | 3       | Controller ➔ Agent       | Revokes a lease (pre-emption or TTL expiry)                                                          |
| `StageDeploymentRequest`    | `0x04`     | `MSG_TYPE_STAGE_DEPLOY_REQ`  | 4       | Controller ➔ Agent       | Stages the candidate artifact into the inactive slot                                                 |
| `StageDeploymentResponse`   | `0x05`     | `MSG_TYPE_STAGE_DEPLOY_RESP` | 5       | Agent ➔ Controller       | Acknowledges staging and the subvolume path                                                          |
| `SlotSwapCommand`           | `0x06`     | `MSG_TYPE_SLOT_SWAP`         | 6       | Controller ➔ Agent       | Atomic slot swap to the candidate slot, then warmup                                                  |
| `RollbackCommand`           | `0x07`     | `MSG_TYPE_ROLLBACK`          | 7       | Controller ➔ Agent       | Restores `restore_slot` immediately                                                                  |
| `IntegrationTestReport`     | `0x08`     | `MSG_TYPE_IT_REPORT`         | 8       | Agent ➔ Controller       | **Final** aggregate gate verdict                                                                     |
| `ArtifactTaintNotification` | `0x09`     | `MSG_TYPE_ARTIFACT_TAINT`    | 9       | Agent ➔ Controller       | Taints a failed artifact digest                                                                      |
| `HealthProbeTelemetry`      | `0x0A`     | `MSG_TYPE_HEALTH_PROBE`      | 10      | Agent ➔ Controller       | Per-tick health probe telemetry                                                                      |
| `HeartbeatTelemetry`        | `0x0B`     | `MSG_TYPE_HEARTBEAT`         | 11      | Agent ⇄ Controller       | Node liveness and free space                                                                         |
| `DeploymentSlot` (enum)     | —          | —                            | 12      | —                        | `SlotA` / `SlotB`                                                                                    |
| `IntegrationTestProgress`   | `0x0C`     | `MSG_TYPE_IT_PROGRESS`       | 13      | Agent ➔ Controller       | **0.2.0.** Per-test gate progress (`Pending`/`Running`/`Passed`/`Failed`, `critical`, `duration_ms`) |
| `TestState` (enum)          | —          | —                            | 14      | —                        | **0.2.0.** State of a single integration test                                                        |
| `ScenarioExecutionPlan`     | `0x0D`     | `MSG_TYPE_SCENARIO_PLAN`     | 15      | Controller ➔ Agent       | **0.3.0.** Authoritative `it_archive_digest`, selected tests, scenarios, env/args, CIDRs, RO mounts  |
| `ScenarioTestSpec`          | —          | —                            | 16      | —                        | **0.3.0.** Per-scenario live probe specification embedded in `ScenarioExecutionPlan`                 |
| `ScenarioEnvVar`            | —          | —                            | 17      | —                        | **0.3.0.** Allowlisted key-value environment entry for sandboxed canary probes                       |
| `ScenarioPlanAck`           | `0x0E`     | `MSG_TYPE_SCENARIO_PLAN_ACK` | 18      | Agent ➔ Controller       | **0.3.0.** Advisory acknowledgement of received `ScenarioExecutionPlan`                              |

### Registry Rules (Additive Only)

- Fory IDs `1..=14` and discriminants `0x01..=0x0C` are **frozen**. New messages are appended with new IDs; existing IDs are never renumbered or reused.
- Receivers dispatch on `msg_type` **before** decoding the payload and skip frames they do not recognize. A `0.3.0` controller writes `0x0D` (`ScenarioExecutionPlan`) immediately followed by `0x06` (`SlotSwapCommand`) without blocking on `0x0E` (`ScenarioPlanAck`), so `0.1.x`/`0.2.x` peers skip unknown frames cleanly; see `tests/it_progress_compat_tests.rs`. Use `is_legacy_msg_type()` / `is_known_msg_type()` to implement the same rule.
- `IntegrationTestReport` stays the final verdict. `IntegrationTestProgress` is advisory live state for UIs and the controller's gate tracker.

### Downstream Dependency Update (Agent and Controller)

[`mu-cd-agent`](https://github.com/LMA1980/mu-cd-agent) and [`mu-cd-controller`](https://github.com/LMA1980/mu-cd-controller) depend on this crate by git branch. To consume a new protocol version:

1. While the IPC change is still an open PR, a downstream feature branch may temporarily point at the IPC feature branch: `mu-cd-ipc = { git = "https://github.com/LMA1980/mu-cd-ipc.git", branch = "feat/<name>" }`.
1. After the IPC PR merges, switch the dependency back to `branch = "main"` and run `cargo update -p mu-cd-ipc` so `Cargo.lock` pins the merged commit.
1. Deploy in order: controller first (it tolerates both old and new agents), then agents. Old controllers skip unknown frames, so either order is safe for 0.2.0.

______________________________________________________________________

## Type Invariants

1. **`usize` vs `u32` Discipline**:
   - `usize` is reserved strictly for local in-memory collections, array indices, and buffer pointer offsets.
   - `u32` is strictly used for wire packet lengths, sequence flags, message types, and network IDs.
1. **Zero-Copy Slicing**:
   - Message payloads are parsed zero-copy from byte buffers; unaligned memory access is forbidden.
1. **Little-Endian Wire Canonicalization**:
   - All multi-byte integer fields are encoded in standard Little-Endian format (`u16`, `u32`, `u64`).

______________________________________________________________________

## License

Licensed under the GNU Lesser General Public License v3.0 with the LGPL-3.0 Linking Exception (LGPL-3.0-only WITH LGPL-3.0-linking-exception). See [LICENSE](LICENSE) for details.
