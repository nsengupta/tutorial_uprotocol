// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Nirmalya Sengupta (https://github.com/nsengupta)

use protobuf::Message;
use up_rust::UMessage;

/// Encodes a UMessage into a Unix-socket compatible framed byte buffer.
///
/// The frame consists of a 4-byte Big-Endian length prefix followed by the
/// protobuf-encoded UMessage payload.
pub fn serialize_for_unix_socket(msg: &UMessage) -> Result<Vec<u8>, anyhow::Error> {
    let envelope_bytes = msg.write_to_bytes()?;

    let msg_len = envelope_bytes.len() as u32;
    let mut framed_buffer = msg_len.to_be_bytes().to_vec();
    framed_buffer.append(&mut envelope_bytes.to_vec()); // Length is prefixed

    Ok(framed_buffer)
}

/// Decodes a framed byte buffer produced by [`serialize_for_unix_socket`].
///
/// Expects the same layout: 4-byte Big-Endian length prefix, then protobuf body.
pub fn deserialize_for_unix_socket(framed: &[u8]) -> Result<UMessage, anyhow::Error> {
    if framed.len() < 4 {
        anyhow::bail!("framed buffer too short for length prefix");
    }

    let body_len = u32::from_be_bytes(framed[0..4].try_into()?) as usize;
    let end = 4 + body_len;
    if framed.len() < end {
        anyhow::bail!("framed buffer too short for declared body length");
    }

    Ok(UMessage::parse_from_bytes(&framed[4..end])?)
}
