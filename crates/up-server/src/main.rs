use tokio::net::UnixListener;
use tokio::io::AsyncReadExt;
use up_rust::UMessage;
use protobuf::Message;
use std::io::{Write, stdout};

const SOCKET_PATH: &str = "/tmp/uprotocol_twin.sock";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    // Clean up dead nodes from previous runs before binding.
    let _ = std::fs::remove_file(SOCKET_PATH);
    let listener = UnixListener::bind(SOCKET_PATH)?;
    println!("uProtocol Socket Server listening on: {}", SOCKET_PATH);

    loop {
        let (mut stream, _) = listener.accept().await?;

        // Spawn a dedicated task for each incoming socket connection.
        tokio::spawn(async move {
            // Step A: Read length prefix header (4 Bytes).
            let mut len_bytes = [0u8; 4];
            if stream.read_exact(&mut len_bytes).await.is_err() {
                return;
            }
            let expected_len = u32::from_be_bytes(len_bytes) as usize;

            // Step B: Read matching body bytes based on length header.
            let mut body_bytes = vec![0u8; expected_len];
            if stream.read_exact(&mut body_bytes).await.is_err() {
                return;
            }

            // Step C: Reconstruct uProtocol semantics by decoding the stream bytes.
            match UMessage::parse_from_bytes(&body_bytes[..]) {
                Ok(u_message) => {
                    if let Some(payload_data) = u_message.payload.as_ref() {
                        let extracted_bytes: Vec<u8> = payload_data.clone().into();
                        let (soc, temp) = unpack_bms_can_frame(&extracted_bytes);

                        let output = format!(
                            "[Digital Twin Server] Processing incoming CAN telemetry stream...\n\
                             -> State of Charge: {}%\n\
                             -> Cell Temp: {} °C",
                            soc, temp,
                        );
                        println!("{}", output);
                        let _ = stdout().flush();
                    }
                }
                Err(e) => eprintln!("Decode error: {:?}", e),
            }
        });
    }
}

fn unpack_bms_can_frame(can_data: &[u8]) -> (f32, i8) {
    if can_data.len() < 2 { return (0.0, 0); }

    // Unpack according to DBC rules
    let raw_soc = can_data[0];
    let battery_level_pct = raw_soc as f32 * 0.5;

    let temperature_c = can_data[1] as i8;

    (battery_level_pct, temperature_c)
}
