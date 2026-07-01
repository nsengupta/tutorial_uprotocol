use tokio::net::UnixStream;
use tokio::io::AsyncWriteExt;
use up_rust::{UUri, UAttributes, UMessage, UMessageType, UPayloadFormat};
use up_rust::communication::UPayload;
use protobuf::MessageField;
use rand::Rng;

use up_frame_codec::serialize_for_unix_socket;

const SOCKET_PATH: &str = "/tmp/uprotocol_twin.sock";

#[tokio::main]
async fn main() -> Result<(), anyhow::Error> {
    println!("--- Battery telemetry publisher starting ---");

    // 1. Build a source uURI for the battery telemetry publisher entity.
    let source_uri = UUri {
        authority_name: "local_vehicle".to_string(),
        ue_id: 0x1010,
        ue_version_major: 1,
        resource_id: 0x8001,
        ..Default::default()
    };

    // 2. Build and send 5 messages with random battery/temperature values.
    let mut rng = rand::rng();

    for i in 1..=5 {
        let battery_pct: f32 = rng.random_range(75.0..78.9);
        let temp_c: i8 = rng.random_range(20..=25);

        println!("Message {}: SoC = {:.1}%, Temp = {}°C", i, battery_pct, temp_c);

        let u_payload = UPayload::new(
            pack_bms_can_frame(battery_pct, temp_c).to_vec(),
            UPayloadFormat::UPAYLOAD_FORMAT_RAW);

        let attributes = UAttributes {
            id: MessageField::from(Some(up_rust::UUID::build())),
            type_: UMessageType::UMESSAGE_TYPE_PUBLISH.into(),
            source: Some(source_uri.clone()).into(),
            ttl: Some(5000),
            ..Default::default()
        };

        let message = UMessage {
            attributes: Some(attributes).into(),
            payload: Some(u_payload.payload()),
            ..Default::default()
        };

        let framed = serialize_for_unix_socket(&message)?;

        let mut stream = UnixStream::connect(SOCKET_PATH).await?;
        stream.write_all(&framed).await?;
        stream.flush().await?;

        println!("   Sent {} bytes.\n", framed.len());
    }

    Ok(())
}

fn pack_bms_can_frame(battery_level_pct: f32, temperature_c: i8) -> [u8; 8] {
    let mut can_data = [0u8; 8];

    // Convert using a DBC scale of 0.5 (e.g. 75.0 / 0.5 = 150)
    let raw_soc = (battery_level_pct / 0.5) as u8;

    can_data[0] = raw_soc;
    can_data[1] = temperature_c as u8;

    can_data
}
