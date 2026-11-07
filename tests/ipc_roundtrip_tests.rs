use mu_cd_ipc::control::{
    create_fory_runtime, ArtifactTaintNotification, DeploymentLeaseGrant, DeploymentLeaseRequest,
    DeploymentLeaseRevoke, DeploymentSlot, HealthProbeTelemetry, HeartbeatTelemetry,
    IntegrationTestReport, RollbackCommand, SlotSwapCommand, StageDeploymentRequest,
    StageDeploymentResponse,
};
use mu_cd_ipc::error::IpcError;
use mu_cd_ipc::framing::{
    read_frame, write_frame, IpcHeader, CURRENT_VERSION, HEADER_SIZE, MAGIC, MAX_FRAME_SIZE,
    MSG_TYPE_ARTIFACT_TAINT, MSG_TYPE_HEALTH_PROBE, MSG_TYPE_HEARTBEAT, MSG_TYPE_IT_REPORT,
    MSG_TYPE_LEASE_GRANT, MSG_TYPE_LEASE_REQ, MSG_TYPE_LEASE_REVOKE, MSG_TYPE_ROLLBACK,
    MSG_TYPE_SLOT_SWAP, MSG_TYPE_STAGE_DEPLOY_REQ, MSG_TYPE_STAGE_DEPLOY_RESP,
};
use tokio::io::{duplex, AsyncWriteExt, DuplexStream};
use zerocopy::IntoBytes;

#[test]
fn test_ipc_header_size_and_alignment() {
    assert_eq!(std::mem::size_of::<IpcHeader>(), 24);
    assert_eq!(std::mem::align_of::<IpcHeader>(), 8);
    assert_eq!(HEADER_SIZE, 24);
}

#[test]
fn test_ipc_header_valid_magic_and_zero_copy_roundtrip() {
    let header = IpcHeader::new(MSG_TYPE_LEASE_REQ, 1024, 0x01, 42);
    assert_eq!(header.magic, MAGIC);
    assert_eq!(header.magic, *b"MCDE");
    assert_eq!(header.version, CURRENT_VERSION);
    assert_eq!(header.msg_type, MSG_TYPE_LEASE_REQ);
    assert_eq!(header.payload_len, 1024);
    assert_eq!(header.flags, 0x01);
    assert_eq!(header.sequence, 42);

    let bytes = header.as_bytes();
    assert_eq!(bytes.len(), 24);

    let (parsed, rest) = IpcHeader::parse(bytes).expect("parse valid header");
    assert_eq!(parsed, header);
    assert!(rest.is_empty());
}

#[test]
fn test_ipc_header_buffer_too_small() {
    for len in 0..HEADER_SIZE {
        let buf = vec![0u8; len];
        let err = IpcHeader::parse(&buf).unwrap_err();
        match err {
            IpcError::BufferTooSmall { expected, actual } => {
                assert_eq!(expected, HEADER_SIZE);
                assert_eq!(actual, len);
            }
            other => panic!("expected BufferTooSmall for len {len}, got {other:?}"),
        }
    }
}

#[test]
fn test_ipc_header_invalid_magic() {
    let mut header = IpcHeader::new(MSG_TYPE_LEASE_REQ, 64, 0, 1);
    header.magic = *b"K3CI";

    let bytes = header.as_bytes();
    let err = IpcHeader::parse(bytes).unwrap_err();
    match err {
        IpcError::InvalidMagic => {}
        other => panic!("expected InvalidMagic, got {other:?}"),
    }
}

#[test]
fn test_ipc_header_unsupported_version() {
    let mut header = IpcHeader::new(MSG_TYPE_LEASE_REQ, 64, 0, 1);
    header.version = 99;

    let bytes = header.as_bytes();
    let err = IpcHeader::parse(bytes).unwrap_err();
    match err {
        IpcError::UnsupportedVersion { expected, actual } => {
            assert_eq!(expected, CURRENT_VERSION);
            assert_eq!(actual, 99);
        }
        other => panic!("expected UnsupportedVersion, got {other:?}"),
    }
}

#[tokio::test]
async fn test_read_frame_oversized_payload_rejected() {
    let (mut client, mut server) = duplex(64);
    let oversized_len = (MAX_FRAME_SIZE + 1) as u32;
    let header = IpcHeader::new(MSG_TYPE_STAGE_DEPLOY_REQ, oversized_len, 0, 100);

    // Write only the 24-byte header; read_frame must reject before payload read
    client
        .write_all(header.as_bytes())
        .await
        .expect("write header");

    let err = read_frame(&mut server).await.unwrap_err();
    match err {
        IpcError::FrameTooLarge { max, actual } => {
            assert_eq!(max, MAX_FRAME_SIZE);
            assert_eq!(actual, MAX_FRAME_SIZE + 1);
        }
        other => panic!("expected FrameTooLarge, got {other:?}"),
    }
}

async fn roundtrip_fory_frame<T>(
    fory: &fory::Fory,
    client: &mut DuplexStream,
    server: &mut DuplexStream,
    msg_type: u16,
    sequence: u64,
    msg: &T,
) where
    T: fory::Serializer<Target = T> + PartialEq + std::fmt::Debug,
{
    let payload = fory.serialize(msg).expect("fory serialize");
    let header = IpcHeader::new(msg_type, payload.len() as u32, 0, sequence);

    write_frame(client, &header, &payload)
        .await
        .expect("write_frame");

    let (read_hdr, read_payload) = read_frame(server).await.expect("read_frame");
    assert_eq!(read_hdr, header);
    assert_eq!(read_payload, payload);

    let decoded: T = fory.deserialize(&read_payload).expect("fory deserialize");
    assert_eq!(&decoded, msg);
}

async fn roundtrip_json_frame<T>(
    client: &mut DuplexStream,
    server: &mut DuplexStream,
    msg_type: u16,
    sequence: u64,
    msg: &T,
) where
    T: serde::Serialize + serde::de::DeserializeOwned + PartialEq + std::fmt::Debug,
{
    let payload = serde_json::to_vec(msg).expect("serde_json serialize");
    let header = IpcHeader::new(msg_type, payload.len() as u32, 0, sequence);

    write_frame(client, &header, &payload)
        .await
        .expect("write_frame");

    let (read_hdr, read_payload) = read_frame(server).await.expect("read_frame");
    assert_eq!(read_hdr, header);
    assert_eq!(read_payload, payload);

    let decoded: T = serde_json::from_slice(&read_payload).expect("serde_json deserialize");
    assert_eq!(&decoded, msg);
}

#[tokio::test]
async fn test_control_messages_roundtrip_all_11_types() {
    let fory = create_fory_runtime().expect("create_fory_runtime");
    let (mut client, mut server) = duplex(128 * 1024);

    // 1. DeploymentLeaseRequest
    let req = DeploymentLeaseRequest {
        deployment_id: "dep-req-1".into(),
        node_id: "node-a".into(),
        service_name: "muconde-agent".into(),
        artifact_sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
        requested_ttl_secs: 300,
    };
    roundtrip_fory_frame(&fory, &mut client, &mut server, MSG_TYPE_LEASE_REQ, 1, &req).await;
    roundtrip_json_frame(&mut client, &mut server, MSG_TYPE_LEASE_REQ, 2, &req).await;

    // 2. DeploymentLeaseGrant (granted)
    let grant_ok = DeploymentLeaseGrant {
        deployment_id: "dep-grant-1".into(),
        lease_id: "lease-101".into(),
        node_id: "node-a".into(),
        active_slot: DeploymentSlot::SlotA,
        candidate_slot: DeploymentSlot::SlotB,
        expires_at_epoch_ms: 1_700_000_100_000,
        granted: true,
        denial_reason: None,
    };
    roundtrip_fory_frame(
        &fory,
        &mut client,
        &mut server,
        MSG_TYPE_LEASE_GRANT,
        3,
        &grant_ok,
    )
    .await;
    roundtrip_json_frame(&mut client, &mut server, MSG_TYPE_LEASE_GRANT, 4, &grant_ok).await;

    // 2. DeploymentLeaseGrant (denied)
    let grant_denied = DeploymentLeaseGrant {
        deployment_id: "dep-grant-2".into(),
        lease_id: "".into(),
        node_id: "node-b".into(),
        active_slot: DeploymentSlot::SlotA,
        candidate_slot: DeploymentSlot::SlotB,
        expires_at_epoch_ms: 0,
        granted: false,
        denial_reason: Some("Active deployment in progress on candidate slot".into()),
    };
    roundtrip_fory_frame(
        &fory,
        &mut client,
        &mut server,
        MSG_TYPE_LEASE_GRANT,
        5,
        &grant_denied,
    )
    .await;
    roundtrip_json_frame(
        &mut client,
        &mut server,
        MSG_TYPE_LEASE_GRANT,
        6,
        &grant_denied,
    )
    .await;

    // 3. DeploymentLeaseRevoke
    let revoke = DeploymentLeaseRevoke {
        deployment_id: "dep-revoke-1".into(),
        lease_id: "lease-101".into(),
        reason: "Operator aborted deployment".into(),
    };
    roundtrip_fory_frame(
        &fory,
        &mut client,
        &mut server,
        MSG_TYPE_LEASE_REVOKE,
        7,
        &revoke,
    )
    .await;
    roundtrip_json_frame(&mut client, &mut server, MSG_TYPE_LEASE_REVOKE, 8, &revoke).await;

    // 4. StageDeploymentRequest
    let stage_req = StageDeploymentRequest {
        deployment_id: "dep-stage-1".into(),
        lease_id: "lease-101".into(),
        service_name: "muconde-agent".into(),
        artifact_sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
        target_slot: DeploymentSlot::SlotB,
    };
    roundtrip_fory_frame(
        &fory,
        &mut client,
        &mut server,
        MSG_TYPE_STAGE_DEPLOY_REQ,
        9,
        &stage_req,
    )
    .await;
    roundtrip_json_frame(
        &mut client,
        &mut server,
        MSG_TYPE_STAGE_DEPLOY_REQ,
        10,
        &stage_req,
    )
    .await;

    // 5. StageDeploymentResponse (staged_ok)
    let stage_resp_ok = StageDeploymentResponse {
        deployment_id: "dep-stage-1".into(),
        target_slot: DeploymentSlot::SlotB,
        btrfs_subvolume_path: "/srv/muconde/slots/slot-b".into(),
        staged_ok: true,
        error_message: None,
    };
    roundtrip_fory_frame(
        &fory,
        &mut client,
        &mut server,
        MSG_TYPE_STAGE_DEPLOY_RESP,
        11,
        &stage_resp_ok,
    )
    .await;
    roundtrip_json_frame(
        &mut client,
        &mut server,
        MSG_TYPE_STAGE_DEPLOY_RESP,
        12,
        &stage_resp_ok,
    )
    .await;

    // 5. StageDeploymentResponse (error)
    let stage_resp_err = StageDeploymentResponse {
        deployment_id: "dep-stage-2".into(),
        target_slot: DeploymentSlot::SlotB,
        btrfs_subvolume_path: "".into(),
        staged_ok: false,
        error_message: Some("BTRFS subvolume snapshot failed: ENOSPC".into()),
    };
    roundtrip_fory_frame(
        &fory,
        &mut client,
        &mut server,
        MSG_TYPE_STAGE_DEPLOY_RESP,
        13,
        &stage_resp_err,
    )
    .await;
    roundtrip_json_frame(
        &mut client,
        &mut server,
        MSG_TYPE_STAGE_DEPLOY_RESP,
        14,
        &stage_resp_err,
    )
    .await;

    // 6. SlotSwapCommand (SlotA -> SlotB)
    let swap = SlotSwapCommand {
        deployment_id: "dep-swap-1".into(),
        lease_id: "lease-101".into(),
        service_name: "muconde-agent".into(),
        from_slot: DeploymentSlot::SlotA,
        to_slot: DeploymentSlot::SlotB,
        systemd_unit: "muconde-agent.service".into(),
        warmup_timeout_secs: 45,
    };
    roundtrip_fory_frame(
        &fory,
        &mut client,
        &mut server,
        MSG_TYPE_SLOT_SWAP,
        15,
        &swap,
    )
    .await;
    roundtrip_json_frame(&mut client, &mut server, MSG_TYPE_SLOT_SWAP, 16, &swap).await;

    // 7. RollbackCommand (SlotA restore on failed artifact SHA-256)
    let rollback = RollbackCommand {
        deployment_id: "dep-swap-1".into(),
        lease_id: "lease-101".into(),
        service_name: "muconde-agent".into(),
        restore_slot: DeploymentSlot::SlotA,
        failed_artifact_sha256: "badbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadb"
            .into(),
        reason: "Post-swap health probe failed on SlotB".into(),
    };
    roundtrip_fory_frame(
        &fory,
        &mut client,
        &mut server,
        MSG_TYPE_ROLLBACK,
        17,
        &rollback,
    )
    .await;
    roundtrip_json_frame(&mut client, &mut server, MSG_TYPE_ROLLBACK, 18, &rollback).await;

    // 8. IntegrationTestReport (passed)
    let report_ok = IntegrationTestReport {
        deployment_id: "dep-it-1".into(),
        service_name: "muconde-agent".into(),
        artifact_sha256: "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855".into(),
        executed_tests: vec![
            "smoke_test".into(),
            "api_endpoint_test".into(),
            "db_migration_test".into(),
        ],
        passed: true,
        failed_tests: vec![],
        duration_ms: 2350,
        log_excerpt: None,
    };
    roundtrip_fory_frame(
        &fory,
        &mut client,
        &mut server,
        MSG_TYPE_IT_REPORT,
        19,
        &report_ok,
    )
    .await;
    roundtrip_json_frame(&mut client, &mut server, MSG_TYPE_IT_REPORT, 20, &report_ok).await;

    // 8. IntegrationTestReport (failed)
    let report_err = IntegrationTestReport {
        deployment_id: "dep-it-2".into(),
        service_name: "muconde-agent".into(),
        artifact_sha256: "badbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadb".into(),
        executed_tests: vec!["smoke_test".into(), "api_endpoint_test".into()],
        passed: false,
        failed_tests: vec!["api_endpoint_test".into()],
        duration_ms: 1200,
        log_excerpt: Some("HTTP 500 Internal Server Error at /healthz".into()),
    };
    roundtrip_fory_frame(
        &fory,
        &mut client,
        &mut server,
        MSG_TYPE_IT_REPORT,
        21,
        &report_err,
    )
    .await;
    roundtrip_json_frame(
        &mut client,
        &mut server,
        MSG_TYPE_IT_REPORT,
        22,
        &report_err,
    )
    .await;

    // 9. ArtifactTaintNotification
    let taint = ArtifactTaintNotification {
        deployment_id: "dep-taint-1".into(),
        service_name: "muconde-agent".into(),
        artifact_sha256: "badbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadbadb".into(),
        tainted_at_epoch_ms: 1_700_000_200_000,
        reason: "Integration test failed: tainted and blacklisted from candidate slot promotion"
            .into(),
        rollback_completed: true,
    };
    roundtrip_fory_frame(
        &fory,
        &mut client,
        &mut server,
        MSG_TYPE_ARTIFACT_TAINT,
        23,
        &taint,
    )
    .await;
    roundtrip_json_frame(
        &mut client,
        &mut server,
        MSG_TYPE_ARTIFACT_TAINT,
        24,
        &taint,
    )
    .await;

    // 10. HealthProbeTelemetry
    let probe = HealthProbeTelemetry {
        deployment_id: "dep-probe-1".into(),
        node_id: "worker-riscv64".into(),
        service_name: "muconde-agent".into(),
        active_slot: DeploymentSlot::SlotB,
        systemd_active: true,
        health_endpoint_ok: true,
        probe_latency_ms: 12,
        timestamp_epoch_ms: 1_700_000_300_000,
    };
    roundtrip_fory_frame(
        &fory,
        &mut client,
        &mut server,
        MSG_TYPE_HEALTH_PROBE,
        25,
        &probe,
    )
    .await;
    roundtrip_json_frame(&mut client, &mut server, MSG_TYPE_HEALTH_PROBE, 26, &probe).await;

    // 11. HeartbeatTelemetry
    let hb = HeartbeatTelemetry {
        node_id: "worker-riscv64".into(),
        active_deployments: 3,
        btrfs_free_bytes: 128 * 1024 * 1024 * 1024,
        timestamp_epoch_ms: 1_700_000_400_000,
    };
    roundtrip_fory_frame(&fory, &mut client, &mut server, MSG_TYPE_HEARTBEAT, 27, &hb).await;
    roundtrip_json_frame(&mut client, &mut server, MSG_TYPE_HEARTBEAT, 28, &hb).await;
}
