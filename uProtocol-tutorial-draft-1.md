
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

Mention that info may be incomplete or incorrect; I am looking for help to fix these.

----

### What shall we build

We will build a small application using uProtocol's up-rust (link here) implementation. Along the 
way, we will clarify the _what_-s and the _why_-s. I believe this is good way to let things fall 
in the appropriate _conceptual_ place.

The application is simple. There exists:

-   An application that can send out ( _publish_ ) two pieces a car's battery telemetry: 
    (a) State-of-charge and (b) Temperature
-   An application that can read these values and print

Each application runs in its own address-space (two different processes, run from separate 
shells on my Linux machine). 

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
Just the payload is incomplete. We have to prepare a message that holds it (the envelope). 
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
We will revisit the fields of this type, later.

Now that 'what' part is done (more or less, a little remains; we will soon see), the next part 
to deal with is 'how'.

These two applications are running on a single Linux host. One of the easiest ways to connect 
them is to use Unix Domain Socket; [UDS](https://en.wikipedia.org/wiki/Unix_domain_socket), in 
short. It is quite easy to send a byte-stream through an USD. However, USD behaves as if it is 
forwarding a stream; the start and end of a message is not interpreted. So, we have to arrange 
for marking these two. One standard way to achieve this is to prefix the buffer with its length. 
The length is an integer (4 bytes); so the receiving application, can read the length first 4 
bytes, and then read _that many_ bytes from the buffer that arrived. 

