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
    IntegrationTestProgress, IntegrationTestReport, RollbackCommand, ScenarioEnvVar,
    ScenarioExecutionPlan, ScenarioPlanAck, ScenarioTestSpec, SlotSwapCommand,
    StageDeploymentRequest, StageDeploymentResponse, TestState, FORY_ID_ARTIFACT_TAINT,
    FORY_ID_DEPLOYMENT_SLOT, FORY_ID_HEALTH_PROBE, FORY_ID_HEARTBEAT, FORY_ID_IT_PROGRESS,
    FORY_ID_IT_REPORT, FORY_ID_LEASE_GRANT, FORY_ID_LEASE_REQ, FORY_ID_LEASE_REVOKE,
    FORY_ID_ROLLBACK, FORY_ID_SCENARIO_ENV_VAR, FORY_ID_SCENARIO_EXECUTION_PLAN,
    FORY_ID_SCENARIO_PLAN, FORY_ID_SCENARIO_PLAN_ACK, FORY_ID_SCENARIO_SPEC,
    FORY_ID_SCENARIO_TEST_SPEC, FORY_ID_SLOT_SWAP, FORY_ID_STAGE_DEPLOY_REQ,
    FORY_ID_STAGE_DEPLOY_RESP, FORY_ID_TEST_STATE,
};
use mu_cd_ipc::framing::{
    is_known_msg_type, is_legacy_msg_type, read_frame, write_frame, IpcHeader, MSG_TYPE_HEARTBEAT,
    MSG_TYPE_IT_PROGRESS, MSG_TYPE_IT_REPORT, MSG_TYPE_LEASE_REQ, MSG_TYPE_SCENARIO_PLAN,
    MSG_TYPE_SCENARIO_PLAN_ACK, MSG_TYPE_SLOT_SWAP,
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
    assert_eq!(FORY_ID_SCENARIO_EXECUTION_PLAN, 15);
    assert_eq!(FORY_ID_SCENARIO_PLAN, 15);
    assert_eq!(FORY_ID_SCENARIO_SPEC, 16);
    assert_eq!(FORY_ID_SCENARIO_TEST_SPEC, 16);
    assert_eq!(FORY_ID_SCENARIO_ENV_VAR, 17);
    assert_eq!(FORY_ID_SCENARIO_PLAN_ACK, 18);

    assert_eq!(MSG_TYPE_IT_PROGRESS, 0x0C);
    assert_eq!(MSG_TYPE_SCENARIO_PLAN, 0x0D);
    assert_eq!(MSG_TYPE_SCENARIO_PLAN_ACK, 0x0E);
    assert!(!is_legacy_msg_type(MSG_TYPE_IT_PROGRESS));
    assert!(!is_legacy_msg_type(MSG_TYPE_SCENARIO_PLAN));
    assert!(!is_legacy_msg_type(MSG_TYPE_SCENARIO_PLAN_ACK));
    assert!(is_known_msg_type(MSG_TYPE_IT_PROGRESS));
    assert!(is_known_msg_type(MSG_TYPE_SCENARIO_PLAN));
    assert!(is_known_msg_type(MSG_TYPE_SCENARIO_PLAN_ACK));
    for t in MSG_TYPE_LEASE_REQ..=MSG_TYPE_HEARTBEAT {
        assert!(is_legacy_msg_type(t));
        assert!(is_known_msg_type(t));
    }
    assert!(!is_known_msg_type(0x00));
    assert!(!is_known_msg_type(0x0F));
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

/// Fory runtime exactly as `create_fory_runtime` built it in 0.2.x (IDs 1–14 only).
fn legacy_v0_2_fory_runtime() -> Fory {
    let mut fory = legacy_v0_1_fory_runtime();
    fory.register::<TestState>(FORY_ID_TEST_STATE).unwrap();
    fory.register::<IntegrationTestProgress>(FORY_ID_IT_PROGRESS)
        .unwrap();
    fory
}

fn sample_scenario_plan() -> ScenarioExecutionPlan {
    let env_url = ScenarioEnvVar {
        key: "MU_SMOKE_TARGET_URL".into(),
        value: "https://service.cluster.internal:8445".into(),
    };
    let env_ca = ScenarioEnvVar {
        key: "MU_SMOKE_TLS_CA_FILE".into(),
        value: "/etc/mu-cd-canary/tls/ca.crt".into(),
    };
    let scenario = ScenarioTestSpec {
        name: "smoke_example_node_a".into(),
        test_binary: "smoke_example".into(),
        args: vec!["--nocapture".into()],
        env: vec![env_url.clone(), env_ca.clone()],
        cidrs: vec!["127.0.0.1/32".into(), "192.0.2.10/32".into()],
        ro_mounts: vec!["/etc/mu-cd-canary/tls/ca.crt".into()],
        critical: true,
        timeout_secs: 10,
    };
    ScenarioExecutionPlan {
        deployment_id: "dep-it-1".into(),
        lease_id: "lease-it-1".into(),
        service_name: "example-service".into(),
        node_id: "node-a".into(),
        policy_commit_sha: "0123456789abcdef0123456789abcdef01234567".into(),
        it_archive_digest: Some(
            "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
        ),
        bundle_id: Some("bundle-001".into()),
        selected_tests: vec![
            "systemd_unit_active".into(),
            "health_endpoint_ready".into(),
            "smoke_example".into(),
        ],
        scenarios: vec![scenario],
        env: vec![env_url, env_ca],
        args: vec!["--nocapture".into()],
        cidrs: vec!["127.0.0.1/32".into(), "192.0.2.10/32".into()],
        ro_mounts: vec!["/etc/mu-cd-canary/tls/ca.crt".into()],
        max_pids: 256,
        max_memlock_kb: 65_536,
        auto_rollback_on_critical: true,
    }
}

#[test]
fn test_scenario_plan_and_ack_fory_and_json_roundtrip() {
    let fory = create_fory_runtime().unwrap();
    let plan = sample_scenario_plan();

    let bytes = fory.serialize(&plan).unwrap();
    let decoded: ScenarioExecutionPlan = fory.deserialize(&bytes).unwrap();
    assert_eq!(decoded, plan);

    let json = serde_json::to_value(&plan).unwrap();
    let from_json: ScenarioExecutionPlan = serde_json::from_value(json).unwrap();
    assert_eq!(from_json, plan);

    for ack in [
        ScenarioPlanAck {
            deployment_id: "dep-it-1".into(),
            lease_id: "lease-it-1".into(),
            accepted: true,
            rejection_reason: None,
        },
        ScenarioPlanAck {
            deployment_id: "dep-it-1".into(),
            lease_id: "lease-it-1".into(),
            accepted: false,
            rejection_reason: Some("missing required test".into()),
        },
    ] {
        let ack_bytes = fory.serialize(&ack).unwrap();
        let decoded_ack: ScenarioPlanAck = fory.deserialize(&ack_bytes).unwrap();
        assert_eq!(decoded_ack, ack);

        let ack_json = serde_json::to_value(&ack).unwrap();
        let ack_from_json: ScenarioPlanAck = serde_json::from_value(ack_json).unwrap();
        assert_eq!(ack_from_json, ack);
    }
}

#[test]
fn test_legacy_v0_1_and_v0_2_runtimes_decode_v0_3_frozen_payloads_and_reject_new_ids() {
    let v0_3 = create_fory_runtime().unwrap();
    let v0_1 = legacy_v0_1_fory_runtime();
    let v0_2 = legacy_v0_2_fory_runtime();

    let report = sample_report();
    let report_bytes = v0_3.serialize(&report).unwrap();
    assert_eq!(
        v0_1.deserialize::<IntegrationTestReport>(&report_bytes)
            .unwrap(),
        report
    );
    assert_eq!(
        v0_2.deserialize::<IntegrationTestReport>(&report_bytes)
            .unwrap(),
        report
    );

    let progress = sample_progress(TestState::Running, 50);
    let progress_bytes = v0_3.serialize(&progress).unwrap();
    assert_eq!(
        v0_2.deserialize::<IntegrationTestProgress>(&progress_bytes)
            .unwrap(),
        progress
    );

    let plan_bytes = v0_3.serialize(&sample_scenario_plan()).unwrap();
    assert!(
        v0_1.deserialize::<ScenarioExecutionPlan>(&plan_bytes)
            .is_err(),
        "0.1.x runtime must not decode ID 15"
    );
    assert!(
        v0_2.deserialize::<ScenarioExecutionPlan>(&plan_bytes)
            .is_err(),
        "0.2.x runtime must not decode ID 15"
    );
}

#[tokio::test]
async fn test_v0_2_peers_skip_scenario_plan_and_ack_frames_without_deadlock() {
    let v0_3 = create_fory_runtime().unwrap();
    let v0_2 = legacy_v0_2_fory_runtime();
    let (mut controller_side, mut agent_side) = duplex(64 * 1024);

    // 0.3.0 controller writes 0x0D (ScenarioExecutionPlan) immediately followed by 0x06 (SlotSwapCommand)
    // without blocking on 0x0E.
    let plan = sample_scenario_plan();
    let plan_payload = v0_3.serialize(&plan).unwrap();
    write_frame(
        &mut controller_side,
        &IpcHeader::new(MSG_TYPE_SCENARIO_PLAN, plan_payload.len() as u32, 0, 1),
        &plan_payload,
    )
    .await
    .unwrap();

    let swap = SlotSwapCommand {
        deployment_id: "dep-it-1".into(),
        lease_id: "lease-it-1".into(),
        service_name: "example-service".into(),
        from_slot: DeploymentSlot::SlotA,
        to_slot: DeploymentSlot::SlotB,
        systemd_unit: "example-service.service".into(),
        warmup_timeout_secs: 30,
    };
    let swap_payload = v0_3.serialize(&swap).unwrap();
    write_frame(
        &mut controller_side,
        &IpcHeader::new(MSG_TYPE_SLOT_SWAP, swap_payload.len() as u32, 0, 2),
        &swap_payload,
    )
    .await
    .unwrap();

    // 0.2.x agent recognises 0x01..=0x0C only, skips 0x0D cleanly, and decodes 0x06.
    let mut agent_skipped = 0usize;
    let decoded_swap = loop {
        let (hdr, payload) = read_frame(&mut agent_side).await.unwrap();
        if hdr.msg_type > MSG_TYPE_IT_PROGRESS {
            agent_skipped += 1;
            continue;
        }
        if hdr.msg_type == MSG_TYPE_SLOT_SWAP {
            break v0_2.deserialize::<SlotSwapCommand>(&payload).unwrap();
        }
    };
    assert_eq!(agent_skipped, 1);
    assert_eq!(decoded_swap, swap);

    // 0.3.0 agent writes advisory 0x0E (ScenarioPlanAck), then 0x0C (progress), then 0x08 (report).
    let ack = ScenarioPlanAck {
        deployment_id: "dep-it-1".into(),
        lease_id: "lease-it-1".into(),
        accepted: true,
        rejection_reason: None,
    };
    let ack_payload = v0_3.serialize(&ack).unwrap();
    write_frame(
        &mut agent_side,
        &IpcHeader::new(MSG_TYPE_SCENARIO_PLAN_ACK, ack_payload.len() as u32, 0, 3),
        &ack_payload,
    )
    .await
    .unwrap();

    let prog_payload = v0_3
        .serialize(&sample_progress(TestState::Passed, 120))
        .unwrap();
    write_frame(
        &mut agent_side,
        &IpcHeader::new(MSG_TYPE_IT_PROGRESS, prog_payload.len() as u32, 0, 4),
        &prog_payload,
    )
    .await
    .unwrap();

    let report_payload = v0_3.serialize(&sample_report()).unwrap();
    write_frame(
        &mut agent_side,
        &IpcHeader::new(MSG_TYPE_IT_REPORT, report_payload.len() as u32, 0, 5),
        &report_payload,
    )
    .await
    .unwrap();

    // 0.2.x controller skips 0x0E cleanly, still consumes 0x0C and 0x08.
    let mut ctrl_skipped = 0usize;
    let mut saw_progress = false;
    let decoded_report = loop {
        let (hdr, payload) = read_frame(&mut controller_side).await.unwrap();
        if hdr.msg_type > MSG_TYPE_IT_PROGRESS {
            ctrl_skipped += 1;
            continue;
        }
        if hdr.msg_type == MSG_TYPE_IT_PROGRESS {
            let _: IntegrationTestProgress = v0_2.deserialize(&payload).unwrap();
            saw_progress = true;
            continue;
        }
        if hdr.msg_type == MSG_TYPE_IT_REPORT {
            break v0_2.deserialize::<IntegrationTestReport>(&payload).unwrap();
        }
    };
    assert_eq!(ctrl_skipped, 1);
    assert!(saw_progress);
    assert_eq!(decoded_report, sample_report());
}
