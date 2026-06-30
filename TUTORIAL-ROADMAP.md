# Roadmap: uProtocol tutorial for SDV (#rustlang) engineers/programmers/enthusiasts

## 🎯 Strategic Thesis
Demonstrate to the Eclipse SDV community and advanced systems engineers that uProtocol is the mandatory semantic layer for Software-Defined Vehicles (SDVs). Prove that while raw transport layers (like local UDS) move bytes perfectly, they are inherently blind to automotive application semantics, data isolation, and location transparency.

---

## 🛠️ Phase 1: The Illusion of Success & The Architectural Wall
*Goal: Introduce the working multi-binary workspace, then immediately break it by introducing a second independent subscriber to expose raw transport limitations.*

- [ ] **1.1 Establish the Baseline (Real Code, already exists and running):** Present the working 
  multi-binary Cargo layout where `crates/up-client` acts as a telemetry publisher, streaming 
  quantized CAN telemetry for Batteries (`SoC`, `Temperature`) wrapped in `UMessage` bytes to 
  `crates/up-server` over a 4-byte Big-Endian length-framed Unix Domain Socket 
  (`/tmp/uprotocol_twin.sock`). Rename up-client as up-battery-telemetry-publisher and up-server 
  as up-telemetry-subscriber.
- [ ] **1.2 Introduce the High-Impact Use Case (The New Subscriber):**
  Introduce a new, non-negotiable automotive requirement: A second independent microservice application—a **Thermal Management Logging Engine**—must tap into that exact same battery telemetry stream simultaneously to monitor cell temperature thresholds.
- [ ] **1.3 Demonstrate "Deep Water" (Pseudo-Code Friction):**
  Write a high-level, chaotic Rust pseudo-code snippet showing what happens when a developer tries to achieve this with raw UDS. Because a Unix stream socket connection is point-to-point and drains the kernel buffer upon read, show the mess of manually tracking a global list of subscriber streams, forcing buffer duplication and message forwarding within the server's core execution loop. Prove that the developer has accidentally started re-inventing a fragile message broker.

---

## 🛠️ Phase 2: The Semantic Epiphany (uProtocol over UDS)
*Goal: Standardize the application layer using uProtocol's core design abstractions to clean up the business logic while retaining the lightweight UDS connection.*

- [ ] **2.1 Elaborate uProtocol Basics & Semantic Benefits:**
  Break down how standardizing on `UUri` (Addressing/Isolation), `UAttributes` (Intent Mapping), and `UPayloadFormat` (Type Enforcement) cleanly decouples *intent* from *wire format*. 
- [ ] **2.2 Refactor with Real Code (The Semantic Fix):**
  Modify your workspace binaries to utilize uProtocol primitives. Introduce the official `UListener` callback model. Show how checking the `UUri` target destination and processing data cleanly eliminates the need for hardcoded offset math in the business logic layer, allowing the subscriber logic to remain declarative.
- [ ] **2.3 Evaluate the Intermediate State:**
  Confirm that while application routing contracts are now well-defined, the transport layer execution is still bottlenecked by a hardcoded point-to-point filesystem file descriptor (`/tmp/uprotocol_twin.sock`).

---

## 🛠️ Phase 3: The Scaling Limit & The Zenoh Payoff
*Goal: Force a physical topology shift to demonstrate location transparency, swap out the UDS transport for Zenoh, and showcase uProtocol L3 level registration for the PUBLISH pattern.*

- [ ] **3.1 Trigger the Network Topology Constraint:**
  Move the battery tracking microservice entity off the high-performance compute node to an external physical Zone ECU connected via Automotive Ethernet. Highlight that the local filesystem socket path `/tmp/uprotocol_twin.sock` is now completely dead and useless.
- [ ] **3.2 Introduce the Specialized Transport Plugin:**
  Explain how an advanced data-space protocol like **Eclipse Zenoh** acts as the perfect supplementary transport backend to achieve seamless network location transparency across distributed network boundaries.
- [ ] **3.3 Swap Transport Layers with Zero Business Logic Impact:**
  Drop the custom `up-frame-codec` local framing loop. Bring in the official `up-client-zenoh-rust` transport crate. 
- [ ] **3.4 Bring in L3 Registration for PUBLISH:**
  Showcase uProtocol’s L3 layer registration mechanics specifically for the `PUBLISH` pattern. Demonstrate the ultimate payoff: because the codebase was designed around uProtocol semantics, your core telemetry loop in `up-client` and your application listener blocks in `up-server` remain 100% untouched while data transits smoothly across the network interface.