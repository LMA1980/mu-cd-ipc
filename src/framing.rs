use crate::error::IpcError;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use zerocopy::{FromBytes, Immutable, IntoBytes, KnownLayout};

pub const MAGIC: [u8; 4] = *b"MCDE";
pub const CURRENT_VERSION: u16 = 1;
pub const MAX_FRAME_SIZE: usize = 16 * 1024 * 1024;
pub const HEADER_SIZE: usize = std::mem::size_of::<IpcHeader>();

// Message Types
pub const MSG_TYPE_LEASE_REQ: u16 = 0x01;
pub const MSG_TYPE_LEASE_GRANT: u16 = 0x02;
pub const MSG_TYPE_LEASE_REVOKE: u16 = 0x03;
pub const MSG_TYPE_STAGE_DEPLOY_REQ: u16 = 0x04;
pub const MSG_TYPE_STAGE_DEPLOY_RESP: u16 = 0x05;
pub const MSG_TYPE_SLOT_SWAP: u16 = 0x06;
pub const MSG_TYPE_ROLLBACK: u16 = 0x07;
pub const MSG_TYPE_IT_REPORT: u16 = 0x08;
pub const MSG_TYPE_ARTIFACT_TAINT: u16 = 0x09;
pub const MSG_TYPE_HEALTH_PROBE: u16 = 0x0A;
pub const MSG_TYPE_HEARTBEAT: u16 = 0x0B;
/// Added in 0.2.0 (additive): per-test [`crate::control::IntegrationTestProgress`] updates.
///
/// Discriminants `0x01..=0x0B` are frozen. Receivers must dispatch on `msg_type` and skip
/// frames whose type they do not recognise (see [`is_legacy_msg_type`]).
pub const MSG_TYPE_IT_PROGRESS: u16 = 0x0C;
/// Added in 0.3.0 (additive): resolved [`crate::control::ScenarioExecutionPlan`] sent by
/// `mu-cd-controller` before [`MSG_TYPE_SLOT_SWAP`] on the swap stream.
pub const MSG_TYPE_SCENARIO_PLAN: u16 = 0x0D;
/// Added in 0.3.0 (additive): advisory [`crate::control::ScenarioPlanAck`] emitted by
/// `mu-cd-agent` upon validating and storing a [`crate::control::ScenarioExecutionPlan`].
pub const MSG_TYPE_SCENARIO_PLAN_ACK: u16 = 0x0E;

/// Returns `true` for the discriminants defined by protocol 0.1.x (`0x01..=0x0B`).
///
/// A 0.1.x peer only understands these types; any other frame (such as
/// [`MSG_TYPE_IT_PROGRESS`], [`MSG_TYPE_SCENARIO_PLAN`], or [`MSG_TYPE_SCENARIO_PLAN_ACK`])
/// is skipped by its dispatch loop without decoding the payload.
pub fn is_legacy_msg_type(msg_type: u16) -> bool {
    (MSG_TYPE_LEASE_REQ..=MSG_TYPE_HEARTBEAT).contains(&msg_type)
}

/// Returns `true` for every discriminant known to this crate version (`0x01..=0x0E`).
pub fn is_known_msg_type(msg_type: u16) -> bool {
    is_legacy_msg_type(msg_type)
        || matches!(
            msg_type,
            MSG_TYPE_IT_PROGRESS | MSG_TYPE_SCENARIO_PLAN | MSG_TYPE_SCENARIO_PLAN_ACK
        )
}

// Flags
pub const FLAG_COMPRESSED: u32 = 0x01;
pub const FLAG_END_OF_STREAM: u32 = 0x02;

#[derive(IntoBytes, FromBytes, KnownLayout, Immutable, Debug, PartialEq, Eq, Clone, Copy)]
#[repr(C, align(8))]
pub struct IpcHeader {
    pub magic: [u8; 4],
    pub version: u16,
    pub msg_type: u16,
    pub payload_len: u32,
    pub flags: u32,
    pub sequence: u64,
}

impl IpcHeader {
    pub fn new(msg_type: u16, payload_len: u32, flags: u32, sequence: u64) -> Self {
        Self {
            magic: MAGIC,
            version: CURRENT_VERSION,
            msg_type,
            payload_len,
            flags,
            sequence,
        }
    }

    pub fn parse(buf: &[u8]) -> Result<(Self, &[u8]), IpcError> {
        if buf.len() < HEADER_SIZE {
            return Err(IpcError::BufferTooSmall {
                expected: HEADER_SIZE,
                actual: buf.len(),
            });
        }

        let (hdr, rest) = Self::read_from_prefix(buf)
            .map_err(|e| IpcError::MalformedHeader(format!("{:?}", e)))?;

        if hdr.magic != MAGIC {
            return Err(IpcError::InvalidMagic);
        }

        if hdr.version != CURRENT_VERSION {
            return Err(IpcError::UnsupportedVersion {
                expected: CURRENT_VERSION,
                actual: hdr.version,
            });
        }

        Ok((hdr, rest))
    }
}

pub async fn read_frame<R: AsyncRead + Unpin>(
    reader: &mut R,
) -> Result<(IpcHeader, Vec<u8>), IpcError> {
    let mut header_buf = [0u8; HEADER_SIZE];
    reader.read_exact(&mut header_buf).await?;

    let (header, _) = IpcHeader::parse(&header_buf)?;

    let payload_len = header.payload_len as usize;
    if payload_len > MAX_FRAME_SIZE {
        return Err(IpcError::FrameTooLarge {
            max: MAX_FRAME_SIZE,
            actual: payload_len,
        });
    }

    let mut payload = vec![0u8; payload_len];
    if payload_len > 0 {
        reader.read_exact(&mut payload).await?;
    }

    Ok((header, payload))
}

pub async fn write_frame<W: AsyncWrite + Unpin>(
    writer: &mut W,
    header: &IpcHeader,
    payload: &[u8],
) -> Result<(), IpcError> {
    writer.write_all(header.as_bytes()).await?;
    if !payload.is_empty() {
        writer.write_all(payload).await?;
    }
    writer.flush().await?;
    Ok(())
}
