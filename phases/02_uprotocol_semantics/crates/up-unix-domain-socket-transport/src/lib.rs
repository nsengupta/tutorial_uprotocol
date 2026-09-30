// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Nirmalya Sengupta (https://github.com/nsengupta)

//! [`UnixDomainSocketTransport`] — a [`UTransport`] over a local Unix Domain Socket.
//!
//! Both processes use the **same** type:
//! - [`UnixDomainSocketTransport::bind`] — owns the socket path, accepts connections,
//!   dispatches to [`UListener`]s via [`UTransport::register_listener`].
//! - [`UnixDomainSocketTransport::connect`] — send-only attachment to that path
//!   ([`UTransport::send`]); listener registration is not available in this mode.
//!
//! Bind vs connect is wire setup for Unix Domain Sockets, not a Client/Server split
//! in the uProtocol application model.

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use protobuf::Message;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::{UnixListener, UnixStream};
use tokio::sync::RwLock;

use up_frame_codec::serialize_for_unix_socket;
use up_rust::{
    ComparableListener, UCode, UListener, UMessage, UStatus, UTransport, UUri,
    verify_filter_criteria,
};

#[derive(Eq, PartialEq, Hash)]
struct RegisteredListener {
    source_filter: UUri,
    sink_filter: Option<UUri>,
    listener: ComparableListener,
}

impl RegisteredListener {
    /// Source filter first, then sink: `None` matches messages that have no sink (typical PUBLISH).
    fn matches_msg_source(&self, msg: &UMessage) -> bool {
        let Some(attribs) = msg.attributes.as_ref() else {
            return false;
        };
        let Some(source) = attribs.source.as_ref() else {
            return false;
        };

        if !self.source_filter.matches(source) {
            return false;
        }

        if let Some(pattern) = &self.sink_filter {
            attribs
                .sink
                .as_ref()
                .is_some_and(|candidate| pattern.matches(candidate))
        } else {
            attribs.sink.is_none()
        }
    }
}

/// L1 transport agent over a Unix Domain Socket (implements [`UTransport`]).
pub struct UnixDomainSocketTransport {
    socket_path: PathBuf,
    listeners: Arc<RwLock<HashSet<RegisteredListener>>>,
    /// `true` after [`Self::bind`] (accept loop running); `false` after [`Self::connect`].
    accepts_connections: bool,
}

impl UnixDomainSocketTransport {
    /// Attach for sending only: connect to `socket_path` on each [`UTransport::send`].
    /// Wrap in `Arc` at the call site when `SimplePublisher` needs a shared handle.
    pub fn connect(socket_path: impl AsRef<Path>) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
            listeners: Arc::new(RwLock::new(HashSet::new())),
            accepts_connections: false,
        }
    }

    /// Bind `socket_path`, spawn the accept/dispatch loop, and return `Self`.
    ///
    /// The accept task shares the listener table (`Arc<RwLock<…>>`), not an `Arc` of this
    /// transport. Creates the parent directory when needed (see [`up_frame_codec::ensure_socket_dir`]).
    ///
    /// `bind` starts accepting immediately; `register_listener` is a later call. A publisher
    /// that connects in that gap is not a dispatch error — an empty table silently drops
    /// the message. Start the subscriber first because the publisher is a short burst.
    pub async fn bind(socket_path: impl AsRef<Path>) -> Result<Self, UStatus> {
        let socket_path = socket_path.as_ref().to_path_buf();
        if let Some(parent) = socket_path.parent() {
            std::fs::create_dir_all(parent).map_err(|err| {
                UStatus::fail_with_code(
                    UCode::INTERNAL,
                    format!(
                        "failed to create socket directory {}: {err}",
                        parent.display()
                    ),
                )
            })?;
        }
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path).map_err(|err| {
            UStatus::fail_with_code(
                UCode::INTERNAL,
                format!(
                    "failed to bind Unix Domain Socket {}: {err}",
                    socket_path.display()
                ),
            )
        })?;

        let listeners = Arc::new(RwLock::new(HashSet::new()));
        let listeners_for_accept = Arc::clone(&listeners);
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let listeners = Arc::clone(&listeners_for_accept);
                tokio::spawn(async move {
                    if let Ok(message) = read_framed_message(stream).await {
                        dispatch_to(&listeners, message).await;
                    }
                });
            }
        });

        Ok(Self {
            socket_path,
            listeners,
            accepts_connections: true,
        })
    }

    /// Socket path this transport is attached to.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    async fn send_framed(&self, message: UMessage) -> Result<(), UStatus> {
        let framed = serialize_for_unix_socket(&message).map_err(|err| {
            UStatus::fail_with_code(UCode::INTERNAL, format!("failed to frame UMessage: {err}"))
        })?;

        let mut stream = UnixStream::connect(&self.socket_path)
            .await
            .map_err(|err| {
                UStatus::fail_with_code(
                    UCode::UNAVAILABLE,
                    format!(
                        "failed to connect to Unix Domain Socket {}: {err}",
                        self.socket_path.display()
                    ),
                )
            })?;

        stream.write_all(&framed).await.map_err(|err| {
            UStatus::fail_with_code(UCode::INTERNAL, format!("failed to write message: {err}"))
        })?;
        stream.flush().await.map_err(|err| {
            UStatus::fail_with_code(UCode::INTERNAL, format!("failed to flush stream: {err}"))
        })?;

        Ok(())
    }
}

/// Snapshot matching listeners, drop the table lock, then invoke callbacks.
async fn dispatch_to(listeners: &RwLock<HashSet<RegisteredListener>>, message: UMessage) {
    let matching: Vec<ComparableListener> = {
        let guard = listeners.read().await;
        guard
            .iter()
            .filter(|registered| registered.matches_msg_source(&message))
            .map(|registered| registered.listener.clone())
            .collect()
    };
    for listener in matching {
        listener.on_receive(message.clone()).await;
    }
}

#[async_trait]
impl UTransport for UnixDomainSocketTransport {
    async fn send(&self, message: UMessage) -> Result<(), UStatus> {
        self.send_framed(message).await
    }

    async fn register_listener(
        &self,
        source_filter: &UUri,
        sink_filter: Option<&UUri>,
        listener: Arc<dyn UListener>,
    ) -> Result<(), UStatus> {
        if !self.accepts_connections {
            return Err(UStatus::fail_with_code(
                UCode::UNIMPLEMENTED,
                "register_listener requires UnixDomainSocketTransport::bind on the receiving side",
            ));
        }

        verify_filter_criteria(source_filter, sink_filter)?;

        let registered = RegisteredListener {
            source_filter: source_filter.to_owned(),
            sink_filter: sink_filter.map(|u| u.to_owned()),
            listener: ComparableListener::new(listener),
        };

        let mut listeners = self.listeners.write().await;
        if listeners.contains(&registered) {
            return Err(UStatus::fail_with_code(
                UCode::ALREADY_EXISTS,
                "listener already registered for filters",
            ));
        }
        listeners.insert(registered);
        Ok(())
    }

    async fn unregister_listener(
        &self,
        source_filter: &UUri,
        sink_filter: Option<&UUri>,
        listener: Arc<dyn UListener>,
    ) -> Result<(), UStatus> {
        if !self.accepts_connections {
            return Err(UStatus::fail_with_code(
                UCode::UNIMPLEMENTED,
                "unregister_listener requires UnixDomainSocketTransport::bind on the receiving side",
            ));
        }

        let registered = RegisteredListener {
            source_filter: source_filter.to_owned(),
            sink_filter: sink_filter.map(|u| u.to_owned()),
            listener: ComparableListener::new(listener),
        };

        let mut listeners = self.listeners.write().await;
        if listeners.remove(&registered) {
            Ok(())
        } else {
            Err(UStatus::fail_with_code(
                UCode::NOT_FOUND,
                "no such listener registered for filters",
            ))
        }
    }
}

async fn read_framed_message(mut stream: UnixStream) -> Result<UMessage, UStatus> {
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes).await.map_err(|err| {
        UStatus::fail_with_code(
            UCode::INTERNAL,
            format!("failed to read length prefix: {err}"),
        )
    })?;
    let body_len = u32::from_be_bytes(len_bytes) as usize;

    let mut body_bytes = vec![0u8; body_len];
    stream.read_exact(&mut body_bytes).await.map_err(|err| {
        UStatus::fail_with_code(
            UCode::INTERNAL,
            format!("failed to read message body: {err}"),
        )
    })?;

    UMessage::parse_from_bytes(&body_bytes).map_err(|err| {
        UStatus::fail_with_code(UCode::INTERNAL, format!("failed to decode UMessage: {err}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use async_trait::async_trait;
    use up_rust::{LocalUriProvider, MockUListener, StaticUriProvider, UMessageBuilder};

    #[tokio::test]
    async fn send_dispatches_to_matching_listener() {
        let socket = std::env::temp_dir().join("uprotocol_unix_domain_socket_transport_test.sock");
        let _ = std::fs::remove_file(&socket);
        const RESOURCE_ID: u16 = 0x8001;
        let uri_provider = StaticUriProvider::new("my_own_car", 0x1010, 0x01);

        let mut mock = MockUListener::new();
        mock.expect_on_receive().times(1).return_const(());
        let listener = Arc::new(mock);

        let server = UnixDomainSocketTransport::bind(&socket).await.unwrap();
        server
            .register_listener(&uri_provider.get_resource_uri(RESOURCE_ID), None, listener)
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = UnixDomainSocketTransport::connect(&socket);
        client
            .send(
                UMessageBuilder::publish(uri_provider.get_resource_uri(RESOURCE_ID))
                    .build()
                    .unwrap(),
            )
            .await
            .unwrap();

        let _ = std::fs::remove_file(&socket);
    }

    struct HoldReadLockListener {
        entered: Arc<tokio::sync::Notify>,
        release: Arc<tokio::sync::Notify>,
    }

    #[async_trait]
    impl UListener for HoldReadLockListener {
        async fn on_receive(&self, _msg: UMessage) {
            self.entered.notify_one();
            self.release.notified().await;
        }
    }

    struct NopListener;

    #[async_trait]
    impl UListener for NopListener {
        async fn on_receive(&self, _msg: UMessage) {}
    }

    #[tokio::test]
    async fn register_listener_is_not_blocked_by_in_flight_on_receive() {
        let socket = std::env::temp_dir().join(format!(
            "uprotocol_unix_domain_socket_lock_canary_{}.sock",
            std::process::id()
        ));
        let _ = std::fs::remove_file(&socket);
        const RESOURCE_ID: u16 = 0x8001;
        let uri_provider = StaticUriProvider::new("my_own_car", 0x1010, 0x01);
        let source_filter = uri_provider.get_resource_uri(RESOURCE_ID);

        let entered = Arc::new(tokio::sync::Notify::new());
        let release = Arc::new(tokio::sync::Notify::new());
        let holding = Arc::new(HoldReadLockListener {
            entered: entered.clone(),
            release: release.clone(),
        });

        let server = UnixDomainSocketTransport::bind(&socket).await.unwrap();
        server
            .register_listener(&source_filter, None, holding)
            .await
            .unwrap();

        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = UnixDomainSocketTransport::connect(&socket);
        client
            .send(
                UMessageBuilder::publish(source_filter.clone())
                    .build()
                    .unwrap(),
            )
            .await
            .unwrap();

        entered.notified().await;

        tokio::time::timeout(
            std::time::Duration::from_millis(500),
            server.register_listener(&source_filter, None, Arc::new(NopListener)),
        )
        .await
        .expect("register_listener waited on in-flight on_receive — drop the listener-table lock before on_receive")
        .unwrap();

        release.notify_one();
        let _ = std::fs::remove_file(&socket);
    }
}
