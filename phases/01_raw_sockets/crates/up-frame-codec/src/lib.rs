use protobuf::Message;
use up_rust::UMessage;

/// Encodes a UMessage into a Unix-socket compatible framed byte buffer.
///
/// The frame consists of a 4-byte Big-Endian length prefix followed by the
/// protobuf-encoded UMessage payload.
pub fn serialize_for_unix_socket(msg: &UMessage) -> Result<Vec<u8>, anyhow::Error> {
    let payload_bytes = msg.write_to_bytes()?;

    let msg_len = payload_bytes.len() as u32;
    let mut framed_buffer = msg_len.to_be_bytes().to_vec();
    framed_buffer.append(&mut payload_bytes.to_vec());

    Ok(framed_buffer)
}
