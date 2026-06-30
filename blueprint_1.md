# uProtocol System Blueprint Manifest
### Building a Low-Level Transport Proof-of-Concept Over Unix Domain Sockets

To truly master an abstraction framework like **uProtocol**, an engineer must peel back the high-level infrastructure and manually map its data structures to lower operating system communication boundaries. This document preserves the architectural layout and technical primitives required to build a localized, custom **uProtocol Layer 1 (uP-L1)** transport implementation using standard Linux **Unix Domain Sockets**.

## The Architectural Intent
By routing pre-baked `up-rust` models over raw streams, we validate that uProtocol functions entirely as a standardized layer of semantics—independent of commercial brokers or complex physical layouts. This configuration acts as the technical baseline for **Part 1** of our Digital Twin tutorial series.

```
+-----------------------------------------------------------------+
|             Local Software Entity / Digital Twin App            |
+-----------------------------------------------------------------+
|                 uProtocol Semantics (uURI, uMessage)            |
+-----------------------------------------------------------------+
|        [Component 1: Frame Serializer / Framing Codec]          |
+-------------------------------+---------------------------------+
                                | Raw Byte Pipeline (Length + Data)
                                v
         [Component 2: Tokio Asynchronous Unix Domain Socket]
                                |
                                v Path: /tmp/uprotocol_twin.sock
```

---

## Component 1: The Frame Serializer (The Codec Layer)

Unix Domain Sockets operate as continuous byte streams (`SOCK_STREAM`). They do not understand packet boundaries or application layer limits. If two uProtocol messages are sent back-to-back, the receiving buffer will bleed them together.

To guarantee protocol parsing integrity, we implement a simple **Explicit Length Framing** header pattern. Every outbound transaction must prepend exactly 4 bytes (a Big-Endian `u32`) containing the byte size of the subsequent serialized structure.

```rust
// Blueprint interface structure for serialization framing
use up_rust::UMessage;
use prost::Message;

fn serialize_framed_message(msg: &UMessage) -> Result<Vec<u8>, anyhow::Error> {
    // 1. Encode the protobuf structure into raw bytes via prost
    let mut payload_bytes = Vec::new();
    msg.encode(&mut payload_bytes)?;

    // 2. Determine raw length
    let msg_len = payload_bytes.len() as u32;

    // 3. Allocate final buffer and prepend length header (Big-Endian format)
    let mut framed_buffer = msg_len.to_be_bytes().to_vec();
    framed_buffer.append(&mut payload_bytes);

    Ok(framed_buffer)
}
```

---

## Component 2: The Async Unix Socket Server (The Runtime Engine)

The server acts as the local vehicle service infrastructure endpoint. It captures the socket binding descriptor, runs a dedicated listener engine loop, decodes incoming message payloads sequentially, and handles the tracking of metadata attributes.

```rust
use tokio::net::UnixListener;
use tokio::io::AsyncReadExt;
use up_rust::UMessage;
use prost::Message;

async fn run_uprotocol_socket_server(socket_path: &str) -> Result<(), anyhow::Error> {
    // Clean up dead nodes from previous runs before binding
    let _ = std::fs::remove_file(socket_path);
    let listener = UnixListener::bind(socket_path)?;
    println!("uProtocol Socket Server listening on: {}", socket_path);
    
    loop {
        let (mut stream, _) = listener.accept().await?;
        
        // Spawn a dedicated thread task for each incoming socket connection
        tokio::spawn(async move {
            // Step A: Read length prefix header (4 Bytes)
            let mut len_bytes = [0u8; 4];
            if stream.read_exact(&mut len_bytes).await.is_err() { 
                return; 
            }
            let expected_len = u32::from_be_bytes(len_bytes) as usize;

            // Step B: Read matching body bytes based on length header
            let mut body_bytes = vec![0u8; expected_len];
            if stream.read_exact(&mut body_bytes).await.is_err() { 
                return; 
            }

            // Step C: Reconstruct uProtocol semantics by decoding the stream bytes
            match UMessage::decode(&body_bytes[..]) {
                Ok(u_message) => {
                    println!("Parsed Message Context Successfully!");
                    println!("Attributes: {:?}", u_message.attributes.as_ref());
                }
                Err(e) => {
                    eprintln!("Failed to parse incoming byte stream into uMessage: {:?}", e);
                }
            }
        });
    }
}
```

---

## Mental Framework Strategy for Your Draft

* **Emphasize Separation of Concerns:** Note how Component 2 reads bytes blindly and only learns it's handling a `UMessage` at Step C. This perfectly echoes how an IP router processes frames without knowing application intents.
* **The "Self-Knowledge" Standard:** Working this out manually forces you to interact with the underlying byte boundaries, putting you in an optimal position to explain the system to the open-source community later.