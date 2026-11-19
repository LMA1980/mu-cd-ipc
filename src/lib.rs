//! Zero-copy IPC and distributed deployment RPC contracts for MuConDe continuous delivery.

#![deny(unsafe_code)]

pub mod control;
pub mod error;
pub mod framing;

pub use control::{
    create_fory_runtime, ArtifactTaintNotification, DeploymentLeaseGrant, DeploymentLeaseRequest,
    DeploymentLeaseRevoke, DeploymentSlot, HealthProbeTelemetry, HeartbeatTelemetry,
    IntegrationTestProgress, IntegrationTestReport, RollbackCommand, SlotSwapCommand,
    StageDeploymentRequest, StageDeploymentResponse, TestState, FORY_ID_ARTIFACT_TAINT,
    FORY_ID_ARTIFACT_TAINT_NOTIFICATION, FORY_ID_DEPLOYMENT_LEASE_GRANT,
    FORY_ID_DEPLOYMENT_LEASE_REQUEST, FORY_ID_DEPLOYMENT_LEASE_REVOKE, FORY_ID_DEPLOYMENT_SLOT,
    FORY_ID_HEALTH_PROBE, FORY_ID_HEALTH_PROBE_TELEMETRY, FORY_ID_HEARTBEAT,
    FORY_ID_HEARTBEAT_TELEMETRY, FORY_ID_INTEGRATION_TEST_PROGRESS,
    FORY_ID_INTEGRATION_TEST_REPORT, FORY_ID_IT_PROGRESS, FORY_ID_IT_REPORT, FORY_ID_LEASE_GRANT,
    FORY_ID_LEASE_REQ, FORY_ID_LEASE_REVOKE, FORY_ID_ROLLBACK, FORY_ID_ROLLBACK_COMMAND,
    FORY_ID_SLOT_SWAP, FORY_ID_SLOT_SWAP_COMMAND, FORY_ID_STAGE_DEPLOYMENT_REQUEST,
    FORY_ID_STAGE_DEPLOYMENT_RESPONSE, FORY_ID_STAGE_DEPLOY_REQ, FORY_ID_STAGE_DEPLOY_RESP,
    FORY_ID_TEST_STATE,
};
pub use error::IpcError;
pub use framing::{
    is_known_msg_type, is_legacy_msg_type, read_frame, write_frame, IpcHeader, CURRENT_VERSION,
    FLAG_COMPRESSED, FLAG_END_OF_STREAM, HEADER_SIZE, MAGIC, MAX_FRAME_SIZE,
    MSG_TYPE_ARTIFACT_TAINT, MSG_TYPE_HEALTH_PROBE, MSG_TYPE_HEARTBEAT, MSG_TYPE_IT_PROGRESS,
    MSG_TYPE_IT_REPORT, MSG_TYPE_LEASE_GRANT, MSG_TYPE_LEASE_REQ, MSG_TYPE_LEASE_REVOKE,
    MSG_TYPE_ROLLBACK, MSG_TYPE_SLOT_SWAP, MSG_TYPE_STAGE_DEPLOY_REQ, MSG_TYPE_STAGE_DEPLOY_RESP,
};

/// Protocol version identifier.
pub const PROTOCOL_VERSION: u32 = 1;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_protocol_version_and_header_size() {
        assert_eq!(PROTOCOL_VERSION, 1);
        assert_eq!(HEADER_SIZE, 24);
    }

    #[test]
    fn test_fory_roundtrip_all_messages() {
        let fory = create_fory_runtime().expect("fory runtime");

        let req = DeploymentLeaseRequest {
            deployment_id: "dep-1".into(),
            node_id: "node-a".into(),
            service_name: "mu-cd-agent".into(),
            artifact_sha256: "abc123".into(),
            requested_ttl_secs: 300,
        };
        let bytes = fory.serialize(&req).expect("serialize req");
        let decoded: DeploymentLeaseRequest = fory.deserialize(&bytes).expect("deserialize req");
        assert_eq!(req, decoded);

        let grant = DeploymentLeaseGrant {
            deployment_id: "dep-1".into(),
            lease_id: "lease-1".into(),
            node_id: "node-a".into(),
            active_slot: DeploymentSlot::SlotA,
            candidate_slot: DeploymentSlot::SlotB,
            expires_at_epoch_ms: 1_700_000_000_000,
            granted: true,
            denial_reason: None,
        };
        let bytes = fory.serialize(&grant).expect("serialize grant");
        let decoded: DeploymentLeaseGrant = fory.deserialize(&bytes).expect("deserialize grant");
        assert_eq!(grant, decoded);

        let revoke = DeploymentLeaseRevoke {
            deployment_id: "dep-1".into(),
            lease_id: "lease-1".into(),
            reason: "preempted".into(),
        };
        let bytes = fory.serialize(&revoke).expect("serialize revoke");
        let decoded: DeploymentLeaseRevoke = fory.deserialize(&bytes).expect("deserialize revoke");
        assert_eq!(revoke, decoded);

        let stage_req = StageDeploymentRequest {
            deployment_id: "dep-1".into(),
            lease_id: "lease-1".into(),
            service_name: "mu-cd-agent".into(),
            artifact_sha256: "abc123".into(),
            target_slot: DeploymentSlot::SlotB,
        };
        let bytes = fory.serialize(&stage_req).expect("serialize stage_req");
        let decoded: StageDeploymentRequest =
            fory.deserialize(&bytes).expect("deserialize stage_req");
        assert_eq!(stage_req, decoded);

        let stage_resp = StageDeploymentResponse {
            deployment_id: "dep-1".into(),
            target_slot: DeploymentSlot::SlotB,
            btrfs_subvolume_path: "/srv/mucd/slots/slot-b".into(),
            staged_ok: true,
            error_message: None,
        };
        let bytes = fory.serialize(&stage_resp).expect("serialize stage_resp");
        let decoded: StageDeploymentResponse =
            fory.deserialize(&bytes).expect("deserialize stage_resp");
        assert_eq!(stage_resp, decoded);

        let swap = SlotSwapCommand {
            deployment_id: "dep-1".into(),
            lease_id: "lease-1".into(),
            service_name: "mu-cd-agent".into(),
            from_slot: DeploymentSlot::SlotA,
            to_slot: DeploymentSlot::SlotB,
            systemd_unit: "mu-cd-agent.service".into(),
            warmup_timeout_secs: 30,
        };
        let bytes = fory.serialize(&swap).expect("serialize swap");
        let decoded: SlotSwapCommand = fory.deserialize(&bytes).expect("deserialize swap");
        assert_eq!(swap, decoded);

        let rollback = RollbackCommand {
            deployment_id: "dep-1".into(),
            lease_id: "lease-1".into(),
            service_name: "mu-cd-agent".into(),
            restore_slot: DeploymentSlot::SlotA,
            failed_artifact_sha256: "abc123".into(),
            reason: "health probe timeout".into(),
        };
        let bytes = fory.serialize(&rollback).expect("serialize rollback");
        let decoded: RollbackCommand = fory.deserialize(&bytes).expect("deserialize rollback");
        assert_eq!(rollback, decoded);

        let report = IntegrationTestReport {
            deployment_id: "dep-1".into(),
            service_name: "mu-cd-agent".into(),
            artifact_sha256: "abc123".into(),
            executed_tests: vec!["test_smoke".into(), "test_api".into()],
            passed: false,
            failed_tests: vec!["test_api".into()],
            duration_ms: 1420,
            log_excerpt: Some("assertion failed".into()),
        };
        let bytes = fory.serialize(&report).expect("serialize report");
        let decoded: IntegrationTestReport = fory.deserialize(&bytes).expect("deserialize report");
        assert_eq!(report, decoded);

        let taint = ArtifactTaintNotification {
            deployment_id: "dep-1".into(),
            service_name: "mu-cd-agent".into(),
            artifact_sha256: "abc123".into(),
            tainted_at_epoch_ms: 1_700_000_005_000,
            reason: "failed post-swap integration test".into(),
            rollback_completed: true,
        };
        let bytes = fory.serialize(&taint).expect("serialize taint");
        let decoded: ArtifactTaintNotification =
            fory.deserialize(&bytes).expect("deserialize taint");
        assert_eq!(taint, decoded);

        let probe = HealthProbeTelemetry {
            deployment_id: "dep-1".into(),
            node_id: "node-a".into(),
            service_name: "mu-cd-agent".into(),
            active_slot: DeploymentSlot::SlotB,
            systemd_active: true,
            health_endpoint_ok: true,
            probe_latency_ms: 4,
            timestamp_epoch_ms: 1_700_000_010_000,
        };
        let bytes = fory.serialize(&probe).expect("serialize probe");
        let decoded: HealthProbeTelemetry = fory.deserialize(&bytes).expect("deserialize probe");
        assert_eq!(probe, decoded);

        let hb = HeartbeatTelemetry {
            node_id: "node-a".into(),
            active_deployments: 2,
            btrfs_free_bytes: 64 * 1024 * 1024 * 1024,
            timestamp_epoch_ms: 1_700_000_015_000,
        };
        let bytes = fory.serialize(&hb).expect("serialize hb");
        let decoded: HeartbeatTelemetry = fory.deserialize(&bytes).expect("deserialize hb");
        assert_eq!(hb, decoded);
    }
}
