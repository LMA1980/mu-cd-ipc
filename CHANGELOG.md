# Changelog

All notable changes to `mu-cd-ipc` are documented here. The crate follows
[Semantic Versioning](https://semver.org/); before 1.0, minor bumps carry additive protocol changes.

## [0.3.0] - Unreleased

### Added

- Additive `ScenarioExecutionPlan`, `ScenarioTestSpec`, `ScenarioEnvVar`, and `ScenarioPlanAck` wire
  contracts carrying resolved `it_archive_digest`, `bundle_id`, `selected_tests`, scenario specs,
  allowlisted env/args, `cidrs`, `ro_mounts`, and sandbox bounds (#33).
- New Fory registration IDs appended after frozen `1..=14`: `FORY_ID_SCENARIO_EXECUTION_PLAN = 15`
  (alias `FORY_ID_SCENARIO_PLAN`), `FORY_ID_SCENARIO_SPEC = 16` (alias `FORY_ID_SCENARIO_TEST_SPEC`),
  `FORY_ID_SCENARIO_ENV_VAR = 17`, and `FORY_ID_SCENARIO_PLAN_ACK = 18`.
- New frame discriminants `MSG_TYPE_SCENARIO_PLAN = 0x0D` and `MSG_TYPE_SCENARIO_PLAN_ACK = 0x0E`,
  with `is_known_msg_type()` extended to `0x01..=0x0E` and `is_legacy_msg_type()` kept at `0x01..=0x0B`.
- Fory/JSON round-trip and `0.1.x`/`0.2.x` frame-skipping compatibility tests covering the non-blocking
  `0x0D` -> `0x06` slot-swap wire sequence.

### Compatibility

- Wire protocol version (`CURRENT_VERSION = 1`), the 24-byte `IpcHeader`, frozen discriminants
  `0x01..=0x0C`, and frozen Fory IDs `1..=14` remain unchanged.
- A `0.3.0` controller writes `0x0D` immediately before `0x06` without blocking for `0x0E`, so `0.2.x`
  agents skip `0x0D` cleanly and `0.2.x` controllers skip `0x0E` cleanly.

## [0.2.0] - Released

### Added

- `IntegrationTestProgress { deployment_id, test_name, critical, state, duration_ms, timestamp_epoch_ms }`:
  per-test progress emitted by `MuConDe-agent` while the integration gate runs (#13, #14, #15).
- `TestState` enum (`Pending`, `Running`, `Passed`, `Failed`; JSON `snake_case`) with `is_terminal()`.
- New fory registration IDs: `FORY_ID_IT_PROGRESS = 13` (`IntegrationTestProgress`) and
  `FORY_ID_TEST_STATE = 14` (`TestState`). IDs `1..=12` are unchanged (#16).
- New frame discriminant `MSG_TYPE_IT_PROGRESS = 0x0C`, plus `is_legacy_msg_type()` and
  `is_known_msg_type()` helpers.
- Round-trip and backward-compatibility tests (`tests/it_progress_compat_tests.rs`) proving a
  0.1.x peer skips `0x0C` frames and still decodes the final `IntegrationTestReport` (#17).

### Compatibility

- Wire protocol version (`CURRENT_VERSION = 1`) and the 24-byte header are unchanged.
- The aggregate `IntegrationTestReport` stays the final gate verdict. Older controllers
  ignore the unknown message type, so the change is phase 1 (side-by-side) of the
  three-phase migration.

## [0.1.0]

- Initial MCDE 24-byte framing and 11 deployment RPC schemas.
