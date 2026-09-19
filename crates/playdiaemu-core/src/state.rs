//! Instant state and persistent storage envelopes.

use crc32fast::Hasher;
use thiserror::Error;

pub const STATE_MAGIC: &[u8; 8] = b"PLAYDIA1";
// Version 2 stores framebuffer pixels as little-endian XRGB8888 words.
// Version 3 adds F2 choice wait frames and timeout destination after the waiting block.
// Version 4 removes the unused firmware identity field.
// Version 5 stores the fractional CD sector pacing phase.
pub const STATE_VERSION: u16 = 5;

#[derive(Debug, Error)]
pub enum SaveStateError {
    #[error("buffer too small")]
    TooSmall,
    #[error("bad magic")]
    BadMagic,
    #[error("unsupported version {0}")]
    BadVersion(u16),
    #[error("checksum mismatch")]
    Checksum,
    #[error("content identity mismatch")]
    Identity,
    #[error("payload truncated")]
    Truncated,
}

/// Immutable identity of loaded disc content.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ContentIdentity {
    pub disc_crc: u32,
    pub disc_sectors: u32,
}

impl ContentIdentity {
    pub fn encode(&self, out: &mut Vec<u8>) {
        out.extend_from_slice(&self.disc_crc.to_le_bytes());
        out.extend_from_slice(&self.disc_sectors.to_le_bytes());
    }

    pub fn decode(buf: &[u8]) -> Option<(Self, usize)> {
        if buf.len() < 8 {
            return None;
        }
        let disc_crc = u32::from_le_bytes(buf[0..4].try_into().ok()?);
        let disc_sectors = u32::from_le_bytes(buf[4..8].try_into().ok()?);
        Some((
            Self {
                disc_crc,
                disc_sectors,
            },
            8,
        ))
    }
}

pub fn crc32(data: &[u8]) -> u32 {
    let mut h = Hasher::new();
    h.update(data);
    h.finalize()
}

/// Build a framed save-state blob: magic|ver|identity|payload_len|payload|crc.
pub fn encode_state(identity: &ContentIdentity, payload: &[u8]) -> Vec<u8> {
    let mut body = Vec::with_capacity(payload.len() + 64);
    identity.encode(&mut body);
    body.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    body.extend_from_slice(payload);

    let mut out = Vec::with_capacity(8 + 2 + body.len() + 4);
    out.extend_from_slice(STATE_MAGIC);
    out.extend_from_slice(&STATE_VERSION.to_le_bytes());
    out.extend_from_slice(&body);
    let c = crc32(&body);
    out.extend_from_slice(&c.to_le_bytes());
    out
}

pub fn decode_state(buf: &[u8], expected: &ContentIdentity) -> Result<Vec<u8>, SaveStateError> {
    if buf.len() < 8 + 2 + 8 + 4 + 4 {
        return Err(SaveStateError::TooSmall);
    }
    if &buf[0..8] != STATE_MAGIC {
        return Err(SaveStateError::BadMagic);
    }
    let ver = u16::from_le_bytes([buf[8], buf[9]]);
    if ver != STATE_VERSION {
        return Err(SaveStateError::BadVersion(ver));
    }
    let body_end = buf.len() - 4;
    let body = &buf[10..body_end];
    let want = u32::from_le_bytes(buf[body_end..].try_into().unwrap());
    if crc32(body) != want {
        return Err(SaveStateError::Checksum);
    }
    let (identity, off) = ContentIdentity::decode(body).ok_or(SaveStateError::Truncated)?;
    if &identity != expected {
        return Err(SaveStateError::Identity);
    }
    if body.len() < off + 4 {
        return Err(SaveStateError::Truncated);
    }
    let plen = u32::from_le_bytes(body[off..off + 4].try_into().unwrap()) as usize;
    let payload = body
        .get(off + 4..off + 4 + plen)
        .ok_or(SaveStateError::Truncated)?;
    Ok(payload.to_vec())
}
