//! `IntegrationTestProgress` (0.2.0) round-trip and backward-compatibility tests.
//!
//! The compatibility contract: a peer built against 0.1.x dispatches on
//! `IpcHeader::msg_type`, only knows `0x01..=0x0B`, and silently skips any other frame.
//! It therefore never tries to decode an `IntegrationTestProgress` payload and still
//! receives the aggregate `IntegrationTestReport` that follows.

use fory::Fory;
use mu_cd_ipc::control::{
    create_fory_runtime, ArtifactTaintNotification, DeploymentLeaseGrant, DeploymentLeaseRequest,
    DeploymentLeaseRevoke, DeploymentSlot, HealthProbeTelemetry, HeartbeatTelemetry,
    IntegrationTestProgress, IntegrationTestReport, RollbackCommand, SlotSwapCommand,
    StageDeploymentRequest, StageDeploymentResponse, TestState, FORY_ID_ARTIFACT_TAINT,
    FORY_ID_DEPLOYMENT_SLOT, FORY_ID_HEALTH_PROBE, FORY_ID_HEARTBEAT, FORY_ID_IT_PROGRESS,
    FORY_ID_IT_REPORT, FORY_ID_LEASE_GRANT, FORY_ID_LEASE_REQ, FORY_ID_LEASE_REVOKE,
    FORY_ID_ROLLBACK, FORY_ID_SLOT_SWAP, FORY_ID_STAGE_DEPLOY_REQ, FORY_ID_STAGE_DEPLOY_RESP,
    FORY_ID_TEST_STATE,
};
use mu_cd_ipc::framing::{
    is_known_msg_type, is_legacy_msg_type, read_frame, write_frame, IpcHeader, MSG_TYPE_HEARTBEAT,
    MSG_TYPE_IT_PROGRESS, MSG_TYPE_IT_REPORT, MSG_TYPE_LEASE_REQ,
};
use tokio::io::duplex;

fn sample_progress(state: TestState, duration_ms: u64) -> IntegrationTestProgress {
    IntegrationTestProgress {
        deployment_id: "dep-it-1".into(),
        test_name: "health_endpoint_ready".into(),
        critical: true,
        state,
        duration_ms,
        timestamp_epoch_ms: 1_700_000_000_123,
    }
}

fn sample_report() -> IntegrationTestReport {
    IntegrationTestReport {
        deployment_id: "dep-it-1".into(),
        service_name: "example-service".into(),
        artifact_sha256: "abc123".into(),
        executed_tests: vec!["health_endpoint_ready".into()],
        passed: true,
        failed_tests: vec![],
        duration_ms: 420,
        log_excerpt: None,
    }
}

/// Fory runtime exactly as `create_fory_runtime` built it in 0.1.0 (IDs 1–12 only).
fn legacy_v0_1_fory_runtime() -> Fory {
    let mut fory = Fory::builder().xlang(false).build();
    fory.register::<DeploymentLeaseRequest>(FORY_ID_LEASE_REQ)
        .unwrap();
    fory.register::<DeploymentLeaseGrant>(FORY_ID_LEASE_GRANT)
        .unwrap();
    fory.register::<DeploymentLeaseRevoke>(FORY_ID_LEASE_REVOKE)
        .unwrap();
    fory.register::<StageDeploymentRequest>(FORY_ID_STAGE_DEPLOY_REQ)
        .unwrap();
    fory.register::<StageDeploymentResponse>(FORY_ID_STAGE_DEPLOY_RESP)
        .unwrap();
    fory.register::<SlotSwapCommand>(FORY_ID_SLOT_SWAP).unwrap();
    fory.register::<RollbackCommand>(FORY_ID_ROLLBACK).unwrap();
    fory.register::<IntegrationTestReport>(FORY_ID_IT_REPORT)
        .unwrap();
    fory.register::<ArtifactTaintNotification>(FORY_ID_ARTIFACT_TAINT)
        .unwrap();
    fory.register::<HealthProbeTelemetry>(FORY_ID_HEALTH_PROBE)
        .unwrap();
    fory.register::<HeartbeatTelemetry>(FORY_ID_HEARTBEAT)
        .unwrap();
    fory.register::<DeploymentSlot>(FORY_ID_DEPLOYMENT_SLOT)
        .unwrap();
    fory
}

#[test]
fn test_new_ids_are_appended_and_existing_ids_frozen() {
    // Frozen 0.1.x registry.
    let frozen = [
        FORY_ID_LEASE_REQ,
        FORY_ID_LEASE_GRANT,
        FORY_ID_LEASE_REVOKE,
        FORY_ID_STAGE_DEPLOY_REQ,
        FORY_ID_STAGE_DEPLOY_RESP,
        FORY_ID_SLOT_SWAP,
        FORY_ID_ROLLBACK,
        FORY_ID_IT_REPORT,
        FORY_ID_ARTIFACT_TAINT,
        FORY_ID_HEALTH_PROBE,
        FORY_ID_HEARTBEAT,
        FORY_ID_DEPLOYMENT_SLOT,
    ];
    assert_eq!(frozen, [1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12]);
    assert_eq!(FORY_ID_IT_PROGRESS, 13);
    assert_eq!(FORY_ID_TEST_STATE, 14);

    assert_eq!(MSG_TYPE_IT_PROGRESS, 0x0C);
    assert!(!is_legacy_msg_type(MSG_TYPE_IT_PROGRESS));
    assert!(is_known_msg_type(MSG_TYPE_IT_PROGRESS));
    for t in MSG_TYPE_LEASE_REQ..=MSG_TYPE_HEARTBEAT {
        assert!(is_legacy_msg_type(t));
        assert!(is_known_msg_type(t));
    }
    assert!(!is_known_msg_type(0x00));
    assert!(!is_known_msg_type(0x0D));
}

#[test]
fn test_it_progress_fory_roundtrip_all_states() {
    let fory = create_fory_runtime().unwrap();
    for (state, dur) in [
        (TestState::Pending, 0),
        (TestState::Running, 12),
        (TestState::Passed, 350),
        (TestState::Failed, 9_001),
    ] {
        let msg = sample_progress(state, dur);
        let bytes = fory.serialize(&msg).unwrap();
        let decoded: IntegrationTestProgress = fory.deserialize(&bytes).unwrap();
        assert_eq!(decoded, msg);
    }
}

#[test]
fn test_it_progress_json_uses_snake_case_state() {
    let msg = sample_progress(TestState::Running, 5);
    let json = serde_json::to_value(&msg).unwrap();
    assert_eq!(json["state"], "running");
    assert_eq!(json["critical"], true);
    let back: IntegrationTestProgress = serde_json::from_value(json).unwrap();
    assert_eq!(back, msg);
}

#[test]
fn test_test_state_terminal() {
    assert!(!TestState::Pending.is_terminal());
    assert!(!TestState::Running.is_terminal());
    assert!(TestState::Passed.is_terminal());
    assert!(TestState::Failed.is_terminal());
    assert_eq!(TestState::default(), TestState::Pending);
}

#[test]
fn test_legacy_runtime_cannot_decode_progress_payload() {
    // Documents *why* 0.1.x peers must skip on msg_type: their fory runtime has no
    // registration for ID 13/14, so decoding the payload is not attempted.
    let new_fory = create_fory_runtime().unwrap();
    let legacy = legacy_v0_1_fory_runtime();
    let bytes = new_fory
        .serialize(&sample_progress(TestState::Passed, 10))
        .unwrap();
    let res: Result<IntegrationTestProgress, _> = legacy.deserialize(&bytes);
    assert!(res.is_err(), "legacy runtime must not know ID 13");
}

#[test]
fn test_legacy_runtime_still_decodes_report_from_new_runtime() {
    let new_fory = create_fory_runtime().unwrap();
    let legacy = legacy_v0_1_fory_runtime();
    let report = sample_report();
    let bytes = new_fory.serialize(&report).unwrap();
    let decoded: IntegrationTestReport = legacy.deserialize(&bytes).unwrap();
    assert_eq!(decoded, report);
}

#[tokio::test]
async fn test_legacy_peer_skips_unknown_progress_frames_and_reads_final_report() {
    let new_fory = create_fory_runtime().unwrap();
    let (mut agent_side, mut controller_side) = duplex(64 * 1024);

    // New agent: Pending → Running → Passed progress frames, then the aggregate report.
    for (seq, state) in [TestState::Pending, TestState::Running, TestState::Passed]
        .into_iter()
        .enumerate()
    {
        let payload = new_fory
            .serialize(&sample_progress(state, seq as u64 * 100))
            .unwrap();
        let hdr = IpcHeader::new(MSG_TYPE_IT_PROGRESS, payload.len() as u32, 0, seq as u64);
        write_frame(&mut agent_side, &hdr, &payload).await.unwrap();
    }
    let report_payload = new_fory.serialize(&sample_report()).unwrap();
    let hdr = IpcHeader::new(MSG_TYPE_IT_REPORT, report_payload.len() as u32, 0, 9);
    write_frame(&mut agent_side, &hdr, &report_payload)
        .await
        .unwrap();

    // Old controller (0.1.x): dispatch loop that only recognises legacy discriminants.
    let legacy = legacy_v0_1_fory_runtime();
    let mut skipped = 0usize;
    let report = loop {
        let (hdr, payload) = read_frame(&mut controller_side).await.unwrap();
        if !is_legacy_msg_type(hdr.msg_type) {
            skipped += 1;
            continue;
        }
        if hdr.msg_type == MSG_TYPE_IT_REPORT {
            let r: IntegrationTestReport = legacy.deserialize(&payload).unwrap();
            break r;
        }
    };
    assert_eq!(skipped, 3);
    assert_eq!(report, sample_report());
}
