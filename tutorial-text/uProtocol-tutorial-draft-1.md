
### Prologue
The story began when I was exploring the world of Eclipse-SDV (link here) out of curiosity. This 
was an area hitherto completely unknown to me. Yet, I was drawn towards it. Why? I have captured 
the reasons here (my blog link here).
One of the technologies that captured my interest was uProtocol (link here). I am familiar with 
the problem it was trying to solve (I have worked in the area of location-agnostic, 
multi-machine-architecture-friendly, network-carried, multiplex-able middleware for a 
good part of my career), but the domain was different. My aim was to understand the landscape 
well, and Eclipse SDV sites helped; so did uProtocol REPO, blogs (Link here of Pete Le Vasseur, 
chief maintainer of uProtocol) and Youtube videos but what I didn't find was a classical 
tutorial; a tutorial which helped a software developer to lay her/his hands on the code to 
solidify the understanding along with the specifications and examples, and helped create a mental 
map of _what was what_. 

So, I decided to write one myself. This tutrial follows how I approached learning uProtocol; 
hopefully, this will be useful for you too.

----

### Chapter 1

### What shall we build

We will build a small application using uProtocol's up-rust (link here) implementation. Along the 
way, we will clarify the _what_-s and the _why_-s. I believe this is good way to let things fall 
in the appropriate _conceptual_ place.

The application is simple. There exists:

-   An application that can send out ( _publish_ ) two pieces a car's battery telemetry: 
    (a) State-of-charge and (b) Temperature; in the code, this is `battery-telemetry-publisher`
-   An application that can read these values and print; in the code, this is `telemetry-subscriber`

Each application runs in its own address-space (two different processes, run from separate 
shells on my Linux machine). 

This tutorial is **Phase-1**. The code lives in `phases/01_raw_sockets/`:

```
phases/01_raw_sockets/
├── Cargo.toml
└── crates/
    ├── up-frame-codec/             # length-prefix serialize + deserialize
    ├── battery-telemetry-publisher # sends UMessage over UDS
    └── telemetry-subscriber        # receives UMessage over UDS
```

A later phase (Phase-2, in `phases/02_uprotocol_semantics/`) will refactor this same
functionality using uProtocol's own L1 transport and L2 communication helpers.

For any such application where two (or more, but that's later) processes converse between 
themselves, two aspects are important (obvious, one may quip):

1. What is shared
2. How is it shared

The 'what' part needs a little more understanding. One application doesn't 'know' the other 
application. Therefore, both the sides have to have complete knowledge of 'what' - the _structure_ 
and the mechanism to _interpret_ the structure. One side cannot change the 'what' without the 
other side being aware of it. They are bound by this pre-condition. 

The 'how' is the aspect of transportation: either side must be equipped to pass through and 
collect from the same transportation facility or 'channels'. This is the other binding factor. 
It is important to note here, that each such 'channel' comes with its own facilities and 
limitations. Again, both the sides have to agree to adjust to and/or abide by these. We will 
revisit this later in this tutorial.

Let's focus on the *what* part first. 2 pieces of data are sent and received:
1) Percent of charge remaining in battery (a `float` value) 
2) Temperature of battery (again, a `float` value)

We assume that these two are packed in a CAN-frame (we don't care where do these originate; not 
important for the tutorial).(Exlplain the CAN-frame part here).

(Paste the function pack_bms_can_frame() here, which produces the array holding these two data)

This array is the _payload_ that is transported (and collected at the other end). We can tansport
this payload as raw-bytes. But, we want to put it in an envelope. We decide to use uProtocol's types to prepare this envelope.

uProtocol provides a type named [UPayload](https://docs.
rs/up-rust/latest/up_rust/communication/struct.UPayload.html) for this. 

```rust
    let u_payload = UPayload::new(
                        pack_bms_can_frame(battery_pct, temp_c).to_vec(), // Battery telemetry data, in CAN buffer
                        UPayloadFormat::UPAYLOAD_FORMAT_RAW   // Indication that it is a RAW buffer
                    );
```
Just the payload is incomplete. We have to prepare a message (the envelope) that holds it . 
uProtocol provides a type for this as well: [UMessage](https://docs.rs/up-rust/latest/up_rust/struct.UMessage.html). 

```rust
    let message = UMessage {
            attributes: Some(attributes).into(),  // We need to prepare this also
            payload: Some(u_payload.payload()),   // Payload from above
            ..Default::default()
        };
```
Without the attributes, the `UMessage` cannot be formed. We make use of this type: [UAttributes](https://docs.rs/up-rust/latest/up_rust/struct.UAttributes.html).

```rust
        let attributes = UAttributes {
                            // Every instance of a UMessage needs an unique ID
                            id: MessageField::from(Some(up_rust::UUID::build())),
                            // Necessary for this application; we will explore more, later in the tutorial
                            type_: UMessageType::UMESSAGE_TYPE_PUBLISH.into(),
                            // Complete information of where is this message prepared; see below
                            source: Some(source_uri.clone()).into(),
                            // LEFT DEFAULT: This is a decoupled PUBLISH broadcast, not an RPC request!
                            // sink: None.into(),
                            // How long is the message going to leave? We will explore more, later in the tutorial
                            ttl: Some(5000),
                            ..Default::default()
                        };
```

`source` is a mandatory field of `UAttributes`. It folds in a `UUri` which is a corenerstone of 
the whole uProtocol landscape. Again, we make use of a [UUri](https://docs.rs/up-rust/latest/up_rust/struct.UUri.html) type.

```rust
    let source_uri = UUri {
                        authority_name: "local_vehicle".to_string(),
                        ue_id: 0x1010,
                        ue_version_major: 1,
                        resource_id: 0x8001,
                        ..Default::default()
                     };
```
We will revisit the fields of these types, later.

Now that 'what' part is done (more or less, a little remains; we will soon see), the next part 
to deal with is 'how'.

These two applications are running on a single Linux host. One of the easiest ways to connect 
them is to use Unix Domain Socket; [UDS](https://en.wikipedia.org/wiki/Unix_domain_socket), in 
short. It is quite easy to send a byte-stream through an USD. However, USD behaves as if it is 
forwarding a stream; the start and end of a message is not interpreted. So, we have to arrange 
for marking these two. One standard way to achieve this is to prefix the buffer with its length. 
The length is an integer (4 bytes); so the receiving application, can read the length first 4 
bytes, and then read _that many_ bytes from the buffer that arrived. 

In the publisher side
```rust
pub fn serialize_for_unix_socket(msg: &UMessage) -> Result<Vec<u8>, anyhow::Error> {
    let envelope_bytes = msg.write_to_bytes()?;
    let msg_len = envelope_bytes.len() as u32;
    let mut framed_buffer = msg_len.to_be_bytes().to_vec();
    framed_buffer.append(&mut envelope_bytes.to_vec()); // Length is prefixed

    Ok(framed_buffer)
}
```
Conversely, in the subscriber side:
```rust
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
```
[Jump to 'How to run' for a sample output](#how-to-run)

So, there we are. A uProtocol Message has been shared between two applications using a 
OS-provided transport facility. 

One important inference that we can draw is that uProtocol's types and operations are **completely 
independent** of the transport facility that is used to  move the messages. We will revisit this 
aspect in the next sections.

----

### Chapter 2

Let's take a good look at types that uProtocol gives us.

This is what we have seen:

- For transporting a `uPayload`, we need to put it in an envelop of a `uMessage`. 
- An `uMessage` envelope requires a `uAttributes` also.
- An `uAttributes` is comprised of an `UUri` (and other elements)
- An `UUri` carries a set of important meta-information

```
╔═══════════╗
║   UUri    ║
╚═══════════╝
```


To understand what `UUri` stands for, let's take a look at how uProtocol identifies a particular 
piece of software component/service anywhere in the car. To refer to State-of-Charge of a 
Battery subsystem, uProtocol allows us to say:

```shell
    //up/local_vehicle/powertrain.battery/1/battery_soc
```

This is a properly-formed URI. The table explains each '/' separated component of the string:

```text
┌──────────┬─────────────────┬──────────────────────┐
│ Field    │ Code Property   │ Human-Readable Name  │
├──────────┼─────────────────┼──────────────────────┤
│ Scheme   │ up              │ (uProtocol-specific) │
│ Authority│ authority_name  │ local_vehicle        │
│ uEntity  │ ue_id           │ powertrain.battery   │
│ Version  │ ue_version_major│ 1                    │
│ uResource│ resource_id     │ battery_soc          │
└──────────┴─────────────────┴──────────────────────┘
```

Note: A _[uEntity](https://github.com/eclipse-uprotocol/up-spec/blob/main/basics/README.adoc#uentity)_ (uProtocol software entity ) is a piece of software deployed somewhere on a network 
host (in our case, the car itself). uEntities are uniquely identified within a system by means of 
the type and version of the service interface that they implement.

The human-readable URI above, is easy to understand and extremely useful in situations like 
diagnostics. For any other uEntity, this stringified URI is an umambiguous reference to a piece 
of data, made available by the uEntity identifiable as _powertrain.battery_. 

But, uProtocol doesn't pass around such an arbitrary-length string. Instead, uProtocol provides 
a type name `UUri`, which carries the same information but in a more compact, wire-ready, 
micro-URI:
```rust
    UUri {
        authority_name: "local_vehicle".to_string(), // The deployment context; who, from where, is providing this 
        ue_id: 0x1010,                               // The unique ID for this uEntity; who 'owns' it
        ue_version_major: 1,                         // API Major Version
        resource_id: 0x8001,                         // Unique ID for this Telemetry resource
        ..Default::default()
    };
```
Same information, in fewer bytes, that can be transported.

**Are those IDs arbitrary**? No, those are not. Those are _Car-level constants_ !

Let's start with _Authority_. It is the root of a tree holding specific resources, laying out a 
_namespace_; just like, 'github.com' defines the root of all resources held in that server. In the seeminly endless 
Internet, if I want to reach a particular source code of mine, I can type in 'www.github.com/nsengupta/personal_project/...' on the browser. The DNS finds where that 'github.com' 
physically is and hands the HTTP request to it. In the same vain, in a SDV-run car, the 
_Authority_ can hold values like "central_compute", or "front_left_zone", or "telematics_store". 
In the entire car, there can be only one "central_compute" or "front_left_zone"; just like in the 
entire Internet, there can be only one 'github.com'!

Inside the _Authority_, there can be more than one uEntities. For example, inside 
'front_left_zone', there may exist 'head-lamp' and 'wheel'. Each of these is assigned its own 
_ue_id_ ; for example, `0x1010` may denote front-left-head-lamp and `0x101F` may denote 
front-left-wheel.

A given uEntity, may hold one or more _resource_(s); the head-lamp may hold a 'switched_on' and 
'current_lux'; the wheel may hold 'tyre_pressure' only. Each such resource has its own 
_resource_id_ ; for example, 'tyre_pressure' may have _resource_id_ of `0xA010` and 
'switched_on' may have _resource_id_ `0x8001` .

So, in order inform any uEntity, if the head-lamp in the front-left of the car is ON, the 
`URI` will be:

```shell
//up/my_own_car/front_head_lamp.left/v1/is_on
```

In the code, the corresponding `UUri` will be:

```rust
    UUri {
        authority_name: "my_own_car".to_string(),
        ue_id: 0x1010,                  // UE_ID of front-left-head.-lamp
        ue_version_major: 1,            // API Major Version
        resource_id: 0x8001,            // RESOURCE_ID of is_on
        ..Default::default()
    };
```

```rust
+--
|
| Rules exist to assign _uEntity_ IDs. Read more about assignment of IDs to uEntities [here]
(https://github.com/eclipse-uprotocol/up-spec/blob/ffca0bc3caf52dec69ea89a24991483a6fd49b47/up-l3/README.adoc#31-uentity-id-ranges).
|
+--
```

```
╔══════════════════╗
║   UAttributes    ║
╚══════════════════╝
```
_UAttributes_ play a very important role in _uProtocol_'s design. It 
- tells the world, what is the purpose of the message, 
- hints at the contents of the message
- augments the up-stream routing logic of the transporters

From the code:

```rust
    let attributes = UAttributes {
            // Every message's uniqueness
            id: MessageField::from(Some(up_rust::UUID::build())),
            // Hint to the transporters, about the purpose of this message (routing logic)
            type_: UMessageType::UMESSAGE_TYPE_PUBLISH.into(),
            // Where this message is originating from
            source: Some(source_uri.clone()).into(),
            
            // LEFT DEFAULT: This is a decoupled PUBLISH broadcast, not an RPC request!
            // sink: None.into(),

            ttl: Some(5000),
            ..Default::default()
        };
```

The complete data-model for _UAttributes_ is [here](https://docs.rs/up-rust/latest/up_rust/struct.UAttributes.html). Here we are using 4 important elements.

Of particular interest, is the `type_`. It is initialized with 
`UMessageType::UMESSAGE_TYPE_PUBLISH`. This is a hint to the transporter that the message 
carries information for letting anybody - who is interested - know. This 'anybody' can be 
another _uEntity_, present somewhere in the car. The transporter can decide how to ensure that 
every such inquisitive _uEntity_ receives it. For other values that `type_` can have, see [here](https://docs.rs/up-rust/latest/up_rust/enum.UMessageType.html).

The `source` is useful for the transporter for determining who all are interested about this 
particular message. Some are waiting to hear what the tyre-pressure is, while some others are 
interested to know if the head-lamp is ON. 

The `ttl` helps decide if the message is of any use, in case of time-sensitive operations on its 
contents. A message may outlive its usefulness if it reaches the inquisitive _uEntity_ too late.

```
╔═══════════════╗
║   UPayload    ║
╚═══════════════╝
```

`UPayload` is a wrapper around the (raw) message payload, along with the corresponding payload 
[format](https://docs.rs/up-rust/latest/up_rust/enum.UPayloadFormat.html). 

```rust
    let u_payload = UPayload::new(
                        pack_bms_can_frame(battery_pct, temp_c).to_vec(),
                        UPayloadFormat::UPAYLOAD_FORMAT_RAW
                    );
```

```
╔══════════════╗
║   UMessage   ║
╚══════════════╝
```

Finally, we come to `UMessage`. It is comprised of `UAttributes` and `UPayload`.

```rust
let message = UMessage {
                    attributes: Some(attributes).into(),
                    payload: Some(u_payload.payload()),
                    ..Default::default()
                };
```

Once we have the `UMessage`, we have to flatten it to a stream of bytes, so that it can be 
transported easily. `UMessage.write_to_bytes()` gives us the series of octets that is 
transportation-ready.

----

### How to run

```shell
# From the repo root
cargo build --manifest-path phases/01_raw_sockets/Cargo.toml

# Terminal 1 — subscriber
cargo run --manifest-path phases/01_raw_sockets/Cargo.toml -p telemetry-subscriber

# Terminal 2 — publisher
cargo run --manifest-path phases/01_raw_sockets/Cargo.toml -p battery-telemetry-publisher
```

Expected output:

```shell
--- Battery telemetry publisher starting ---
Message 1: SoC = 76.6%, Temp = 24°C
   Sent 67 bytes.

Message 2: SoC = 77.9%, Temp = 23°C
   Sent 67 bytes.

Message 3: SoC = 76.6%, Temp = 21°C
   Sent 67 bytes.

Message 4: SoC = 75.3%, Temp = 20°C
   Sent 67 bytes.

Message 5: SoC = 78.3%, Temp = 22°C
   Sent 67 bytes.

```

```shell
Battery telemetry subscriber listening on: /tmp/uprotocol_twin.sock
[Battery telemetry subscriber] Processing incoming CAN telemetry...
-> State of Charge: 76.5%
-> Cell Temp: 24 °C
[Battery telemetry subscriber] Processing incoming CAN telemetry...
-> State of Charge: 77.5%
-> Cell Temp: 23 °C
[Battery telemetry subscriber] Processing incoming CAN telemetry...
-> State of Charge: 76.5%
-> Cell Temp: 21 °C
[Battery telemetry subscriber] Processing incoming CAN telemetry...
-> State of Charge: 75.0%
-> Cell Temp: 20 °C
[Battery telemetry subscriber] Processing incoming CAN telemetry...
-> State of Charge: 78.0%
-> Cell Temp: 22 °C

```
