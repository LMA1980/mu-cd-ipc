use crate::error::IpcError;
use fory::{Fory, ForyEnum, ForyStruct};
use serde::{Deserialize, Serialize};

pub const FORY_ID_LEASE_REQ: u32 = 1;
pub const FORY_ID_LEASE_GRANT: u32 = 2;
pub const FORY_ID_LEASE_REVOKE: u32 = 3;
pub const FORY_ID_STAGE_DEPLOY_REQ: u32 = 4;
pub const FORY_ID_STAGE_DEPLOY_RESP: u32 = 5;
pub const FORY_ID_SLOT_SWAP: u32 = 6;
pub const FORY_ID_ROLLBACK: u32 = 7;
pub const FORY_ID_IT_REPORT: u32 = 8;
pub const FORY_ID_ARTIFACT_TAINT: u32 = 9;
pub const FORY_ID_HEALTH_PROBE: u32 = 10;
pub const FORY_ID_HEARTBEAT: u32 = 11;
pub const FORY_ID_DEPLOYMENT_SLOT: u32 = 12;
/// Added in 0.2.0 (additive). IDs 1–12 above are frozen and must never be renumbered.
pub const FORY_ID_IT_PROGRESS: u32 = 13;
/// Added in 0.2.0 (additive). Registration ID for the [`TestState`] enum.
pub const FORY_ID_TEST_STATE: u32 = 14;
/// Added in 0.3.0 (additive). IDs 1–14 above are frozen and must never be renumbered.
pub const FORY_ID_SCENARIO_EXECUTION_PLAN: u32 = 15;
/// Added in 0.3.0 (additive). Registration ID for [`ScenarioTestSpec`].
pub const FORY_ID_SCENARIO_SPEC: u32 = 16;
/// Added in 0.3.0 (additive). Registration ID for [`ScenarioEnvVar`].
pub const FORY_ID_SCENARIO_ENV_VAR: u32 = 17;
/// Added in 0.3.0 (additive). Registration ID for [`ScenarioPlanAck`].
pub const FORY_ID_SCENARIO_PLAN_ACK: u32 = 18;

// Full-name aliases for ergonomic downstream imports
pub const FORY_ID_DEPLOYMENT_LEASE_REQUEST: u32 = FORY_ID_LEASE_REQ;
pub const FORY_ID_DEPLOYMENT_LEASE_GRANT: u32 = FORY_ID_LEASE_GRANT;
pub const FORY_ID_DEPLOYMENT_LEASE_REVOKE: u32 = FORY_ID_LEASE_REVOKE;
pub const FORY_ID_STAGE_DEPLOYMENT_REQUEST: u32 = FORY_ID_STAGE_DEPLOY_REQ;
pub const FORY_ID_STAGE_DEPLOYMENT_RESPONSE: u32 = FORY_ID_STAGE_DEPLOY_RESP;
pub const FORY_ID_SLOT_SWAP_COMMAND: u32 = FORY_ID_SLOT_SWAP;
pub const FORY_ID_ROLLBACK_COMMAND: u32 = FORY_ID_ROLLBACK;
pub const FORY_ID_INTEGRATION_TEST_REPORT: u32 = FORY_ID_IT_REPORT;
pub const FORY_ID_ARTIFACT_TAINT_NOTIFICATION: u32 = FORY_ID_ARTIFACT_TAINT;
pub const FORY_ID_HEALTH_PROBE_TELEMETRY: u32 = FORY_ID_HEALTH_PROBE;
pub const FORY_ID_HEARTBEAT_TELEMETRY: u32 = FORY_ID_HEARTBEAT;
pub const FORY_ID_INTEGRATION_TEST_PROGRESS: u32 = FORY_ID_IT_PROGRESS;
pub const FORY_ID_SCENARIO_PLAN: u32 = FORY_ID_SCENARIO_EXECUTION_PLAN;
pub const FORY_ID_SCENARIO_TEST_SPEC: u32 = FORY_ID_SCENARIO_SPEC;

#[derive(ForyEnum, Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, Default)]
pub enum DeploymentSlot {
    #[default]
    SlotA,
    SlotB,
}

#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeploymentLeaseRequest {
    pub deployment_id: String,
    pub node_id: String,
    pub service_name: String,
    pub artifact_sha256: String,
    pub requested_ttl_secs: u64,
}

#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeploymentLeaseGrant {
    pub deployment_id: String,
    pub lease_id: String,
    pub node_id: String,
    pub active_slot: DeploymentSlot,
    pub candidate_slot: DeploymentSlot,
    pub expires_at_epoch_ms: u64,
    pub granted: bool,
    pub denial_reason: Option<String>,
}

#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct DeploymentLeaseRevoke {
    pub deployment_id: String,
    pub lease_id: String,
    pub reason: String,
}

#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct StageDeploymentRequest {
    pub deployment_id: String,
    pub lease_id: String,
    pub service_name: String,
    pub artifact_sha256: String,
    pub target_slot: DeploymentSlot,
}

#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct StageDeploymentResponse {
    pub deployment_id: String,
    pub target_slot: DeploymentSlot,
    pub btrfs_subvolume_path: String,
    pub staged_ok: bool,
    pub error_message: Option<String>,
}

#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct SlotSwapCommand {
    pub deployment_id: String,
    pub lease_id: String,
    pub service_name: String,
    pub from_slot: DeploymentSlot,
    pub to_slot: DeploymentSlot,
    pub systemd_unit: String,
    pub warmup_timeout_secs: u64,
}

#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct RollbackCommand {
    pub deployment_id: String,
    pub lease_id: String,
    pub service_name: String,
    pub restore_slot: DeploymentSlot,
    pub failed_artifact_sha256: String,
    pub reason: String,
}

#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct IntegrationTestReport {
    pub deployment_id: String,
    pub service_name: String,
    pub artifact_sha256: String,
    pub executed_tests: Vec<String>,
    pub passed: bool,
    pub failed_tests: Vec<String>,
    pub duration_ms: u64,
    pub log_excerpt: Option<String>,
}

/// Live state of a single integration test inside a deployment gate.
#[derive(ForyEnum, Serialize, Deserialize, Debug, PartialEq, Eq, Clone, Copy, Default, Hash)]
#[serde(rename_all = "snake_case")]
pub enum TestState {
    /// Declared by the contract but not started yet.
    #[default]
    Pending,
    /// Currently executing on the candidate slot.
    Running,
    /// Completed successfully.
    Passed,
    /// Completed with a failure.
    Failed,
}

impl TestState {
    /// Returns `true` once the test has reached a final state (`Passed` or `Failed`).
    pub fn is_terminal(self) -> bool {
        matches!(self, Self::Passed | Self::Failed)
    }
}

/// Per-test progress update emitted by `MuConDe-agent` while the integration gate runs.
///
/// Sent as [`crate::framing::MSG_TYPE_IT_PROGRESS`] frames before the aggregate
/// [`IntegrationTestReport`], which remains the final verdict. Peers built against
/// 0.1.x do not know this message type and skip the frame (they dispatch on
/// `IpcHeader::msg_type` before attempting to decode the payload).
#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct IntegrationTestProgress {
    pub deployment_id: String,
    pub test_name: String,
    /// `true` when a failure of this test must block promotion (`[C]` in the console).
    pub critical: bool,
    pub state: TestState,
    /// Elapsed time for this test; `0` while `Pending`.
    pub duration_ms: u64,
    pub timestamp_epoch_ms: u64,
}

#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct ArtifactTaintNotification {
    pub deployment_id: String,
    pub service_name: String,
    pub artifact_sha256: String,
    pub tainted_at_epoch_ms: u64,
    pub reason: String,
    pub rollback_completed: bool,
}

#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct HealthProbeTelemetry {
    pub deployment_id: String,
    pub node_id: String,
    pub service_name: String,
    pub active_slot: DeploymentSlot,
    pub systemd_active: bool,
    pub health_endpoint_ok: bool,
    pub probe_latency_ms: u32,
    pub timestamp_epoch_ms: u64,
}

#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Clone)]
pub struct HeartbeatTelemetry {
    pub node_id: String,
    pub active_deployments: u32,
    pub btrfs_free_bytes: u64,
    pub timestamp_epoch_ms: u64,
}

/// Key-value environment variable entry passed to a sandboxed canary scenario.
#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct ScenarioEnvVar {
    pub key: String,
    pub value: String,
}

/// Resolved per-scenario execution specification for a sandboxed live probe.
#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct ScenarioTestSpec {
    pub name: String,
    pub test_binary: String,
    pub args: Vec<String>,
    pub env: Vec<ScenarioEnvVar>,
    pub cidrs: Vec<String>,
    pub ro_mounts: Vec<String>,
    pub critical: bool,
    pub timeout_secs: u64,
}

/// Authoritative per-node integration test and canary scenario plan transmitted by
/// `mu-cd-controller` ahead of [`SlotSwapCommand`] (`MSG_TYPE_SCENARIO_PLAN`, `0x0D`).
///
/// Peers built against `0.1.x`/`0.2.x` do not recognize `0x0D` and skip the frame cleanly
/// before decoding the subsequent `SlotSwapCommand`.
#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct ScenarioExecutionPlan {
    pub deployment_id: String,
    pub lease_id: String,
    pub service_name: String,
    pub node_id: String,
    pub policy_commit_sha: String,
    pub it_archive_digest: Option<String>,
    pub bundle_id: Option<String>,
    pub selected_tests: Vec<String>,
    pub scenarios: Vec<ScenarioTestSpec>,
    pub env: Vec<ScenarioEnvVar>,
    pub args: Vec<String>,
    pub cidrs: Vec<String>,
    pub ro_mounts: Vec<String>,
    pub max_pids: u32,
    pub max_memlock_kb: u64,
    pub auto_rollback_on_critical: bool,
}

/// Advisory acknowledgement emitted by `mu-cd-agent` upon validating and storing a
/// [`ScenarioExecutionPlan`] (`MSG_TYPE_SCENARIO_PLAN_ACK`, `0x0E`).
#[derive(ForyStruct, Serialize, Deserialize, Debug, PartialEq, Eq, Clone)]
pub struct ScenarioPlanAck {
    pub deployment_id: String,
    pub lease_id: String,
    pub accepted: bool,
    pub rejection_reason: Option<String>,
}

pub fn create_fory_runtime() -> Result<Fory, IpcError> {
    let mut fory = Fory::builder().xlang(false).build();
    fory.register::<DeploymentLeaseRequest>(FORY_ID_LEASE_REQ)?;
    fory.register::<DeploymentLeaseGrant>(FORY_ID_LEASE_GRANT)?;
    fory.register::<DeploymentLeaseRevoke>(FORY_ID_LEASE_REVOKE)?;
    fory.register::<StageDeploymentRequest>(FORY_ID_STAGE_DEPLOY_REQ)?;
    fory.register::<StageDeploymentResponse>(FORY_ID_STAGE_DEPLOY_RESP)?;
    fory.register::<SlotSwapCommand>(FORY_ID_SLOT_SWAP)?;
    fory.register::<RollbackCommand>(FORY_ID_ROLLBACK)?;
    fory.register::<IntegrationTestReport>(FORY_ID_IT_REPORT)?;
    fory.register::<ArtifactTaintNotification>(FORY_ID_ARTIFACT_TAINT)?;
    fory.register::<HealthProbeTelemetry>(FORY_ID_HEALTH_PROBE)?;
    fory.register::<HeartbeatTelemetry>(FORY_ID_HEARTBEAT)?;
    fory.register::<DeploymentSlot>(FORY_ID_DEPLOYMENT_SLOT)?;
    // 0.2.0 additions: appended with new IDs, never renumbering the frozen set above.
    fory.register::<TestState>(FORY_ID_TEST_STATE)?;
    fory.register::<IntegrationTestProgress>(FORY_ID_IT_PROGRESS)?;
    // 0.3.0 additions: appended with IDs 15..=18, never renumbering frozen IDs 1..=14.
    fory.register::<ScenarioEnvVar>(FORY_ID_SCENARIO_ENV_VAR)?;
    fory.register::<ScenarioTestSpec>(FORY_ID_SCENARIO_SPEC)?;
    fory.register::<ScenarioExecutionPlan>(FORY_ID_SCENARIO_EXECUTION_PLAN)?;
    fory.register::<ScenarioPlanAck>(FORY_ID_SCENARIO_PLAN_ACK)?;
    Ok(fory)
}
