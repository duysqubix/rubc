pub const MAGIC: &[u8; 4] = b"RUSV";
pub const VERSION: u16 = 2;

pub fn encode_payload(payload: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(6 + payload.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    bytes.extend_from_slice(payload);
    bytes
}

pub fn decode_payload_with_version(bytes: &[u8]) -> crate::Result<(u16, &[u8])> {
    anyhow::ensure!(bytes.len() >= 6, "truncated save state");
    anyhow::ensure!(&bytes[0..4] == MAGIC, "invalid save-state magic");
    let version = u16::from_le_bytes([bytes[4], bytes[5]]);
    anyhow::ensure!(
        version == 1 || version == VERSION,
        "unsupported save-state version"
    );
    Ok((version, &bytes[6..]))
}
