// SPDX-License-Identifier: MIT
// Copyright (c) 2026 Nirmalya Sengupta (https://github.com/nsengupta)

//! UDS-backed [`UTransport`] for local length-framed uProtocol messages.
//!
//! - [`UdsTransport::serve`] binds a Unix socket, accepts connections, and dispatches
//!   decoded [`UMessage`] values to registered [`UListener`] callbacks.
//! - [`UdsTransportClient`] connects per send (matching the Stage 1 publisher pattern).

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
    verify_filter_criteria, ComparableListener, UCode, UListener, UMessage, UStatus, UTransport,
    UUri,
};

#[derive(Eq, PartialEq, Hash)]
struct RegisteredListener {
    source_filter: UUri,
    sink_filter: Option<UUri>,
    listener: ComparableListener,
}

impl RegisteredListener {
    fn matches_msg(&self, msg: &UMessage) -> bool {
        let Some(attribs) = msg.attributes.as_ref() else {
            return false;
        };
        let Some(source) = attribs.source.as_ref() else {
            return false;
        };

        // Check the source: does this message's source URI match the registered filter?
        if !self.source_filter.matches(source) {
            return false;
        }

        // Check the sink: if the registration has a sink filter, the message must
        // carry a matching sink URI. If no sink filter was registered, the message
        // must carry no sink (i.e. it's a broadcast/notification, not an RPC reply).
        if let Some(pattern) = &self.sink_filter {
            attribs.sink.as_ref().is_some_and(|candidate| pattern.matches(candidate))
        } else {
            attribs.sink.is_none()
        }
    }

    async fn on_receive(&self, msg: UMessage) {
        self.listener.on_receive(msg).await;
    }
}

/// Server-side UDS transport: binds a socket path and dispatches to listeners.
pub struct UdsTransport {
    socket_path: PathBuf,
    listeners: Arc<RwLock<HashSet<RegisteredListener>>>,
}

impl UdsTransport {
    /// Bind `socket_path`, spawn the accept/dispatch loop, and return a shareable handle.
    pub async fn serve(socket_path: impl AsRef<Path>) -> Result<Arc<Self>, UStatus> {
        let socket_path = socket_path.as_ref().to_path_buf();
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path).map_err(|err| {
            UStatus::fail_with_code(
                UCode::INTERNAL,
                format!("failed to bind Unix socket {}: {err}", socket_path.display()),
            )
        })?;

        let transport = Arc::new(Self {
            socket_path: socket_path.clone(),
            listeners: Arc::new(RwLock::new(HashSet::new())),
        });

        let dispatch_transport = Arc::clone(&transport);
        tokio::spawn(async move {
            loop {
                let Ok((stream, _)) = listener.accept().await else {
                    continue;
                };
                let transport = Arc::clone(&dispatch_transport);
                tokio::spawn(async move {
                    if let Ok(message) = read_framed_message(stream).await {
                        transport.dispatch(message).await;
                    }
                });
            }
        });

        Ok(transport)
    }

    /// Socket path this server is bound to.
    pub fn socket_path(&self) -> &Path {
        &self.socket_path
    }

    async fn dispatch(&self, message: UMessage) {
        let listeners = self.listeners.read().await;
        for registered in listeners.iter() {
            // Only forward this message to listeners whose registered source/sink
            // filters match the message's own source and (optional) sink URIs.
            // This is uProtocol's URI-based filter mechanism — the transport delivers
            // only to subscribers whose registered URI patterns match the message's source.
            // Unlike a broker topic model, this is a direct URI match over a stream socket.
            if registered.matches_msg(&message) {
                registered.on_receive(message.clone()).await;
            }
        }
    }
}

#[async_trait]
impl UTransport for UdsTransport {
    async fn send(&self, message: UMessage) -> Result<(), UStatus> {
        UdsTransportClient::new(&self.socket_path).send(message).await
    }

    async fn register_listener(
        &self,
        source_filter: &UUri,
        sink_filter: Option<&UUri>,
        listener: Arc<dyn UListener>,
    ) -> Result<(), UStatus> {
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

/// Client-side UDS transport: opens a fresh connection per [`UTransport::send`].
pub struct UdsTransportClient {
    socket_path: PathBuf,
}

impl UdsTransportClient {
    pub fn new(socket_path: impl AsRef<Path>) -> Self {
        Self {
            socket_path: socket_path.as_ref().to_path_buf(),
        }
    }
}

#[async_trait]
impl UTransport for UdsTransportClient {
    async fn send(&self, message: UMessage) -> Result<(), UStatus> {
        let framed = serialize_for_unix_socket(&message).map_err(|err| {
            UStatus::fail_with_code(
                UCode::INTERNAL,
                format!("failed to frame UMessage: {err}"),
            )
        })?;

        let mut stream = UnixStream::connect(&self.socket_path).await.map_err(|err| {
            UStatus::fail_with_code(
                UCode::UNAVAILABLE,
                format!(
                    "failed to connect to Unix socket {}: {err}",
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

    async fn register_listener(
        &self,
        _source_filter: &UUri,
        _sink_filter: Option<&UUri>,
        _listener: Arc<dyn UListener>,
    ) -> Result<(), UStatus> {
        Err(UStatus::fail_with_code(
            UCode::UNIMPLEMENTED,
            "listener registration requires UdsTransport::serve on the receiving side",
        ))
    }

    async fn unregister_listener(
        &self,
        _source_filter: &UUri,
        _sink_filter: Option<&UUri>,
        _listener: Arc<dyn UListener>,
    ) -> Result<(), UStatus> {
        Err(UStatus::fail_with_code(
            UCode::UNIMPLEMENTED,
            "listener registration requires UdsTransport::serve on the receiving side",
        ))
    }
}

async fn read_framed_message(mut stream: UnixStream) -> Result<UMessage, UStatus> {
    let mut len_bytes = [0u8; 4];
    stream.read_exact(&mut len_bytes).await.map_err(|err| {
        UStatus::fail_with_code(UCode::INTERNAL, format!("failed to read length prefix: {err}"))
    })?;
    let body_len = u32::from_be_bytes(len_bytes) as usize;

    let mut body_bytes = vec![0u8; body_len];
    stream.read_exact(&mut body_bytes).await.map_err(|err| {
        UStatus::fail_with_code(UCode::INTERNAL, format!("failed to read message body: {err}"))
    })?;

    UMessage::parse_from_bytes(&body_bytes).map_err(|err| {
        UStatus::fail_with_code(UCode::INTERNAL, format!("failed to decode UMessage: {err}"))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use up_rust::{LocalUriProvider, MockUListener, StaticUriProvider, UMessageBuilder};

    const SOCKET: &str = "/tmp/uprotocol_uds_transport_test.sock";

    #[tokio::test]
    async fn send_dispatches_to_matching_listener() {
        let _ = std::fs::remove_file(SOCKET);
        const RESOURCE_ID: u16 = 0x8001;
        let uri_provider = StaticUriProvider::new("local_vehicle", 0x1010, 0x01);

        let mut mock = MockUListener::new();
        mock.expect_on_receive().times(1).return_const(());
        let listener = Arc::new(mock);

        let server = UdsTransport::serve(SOCKET).await.unwrap();
        server
            .register_listener(
                &uri_provider.get_resource_uri(RESOURCE_ID),
                None,
                listener,
            )
            .await
            .unwrap();

        // Allow the accept loop to bind.
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;

        let client = UdsTransportClient::new(SOCKET);
        client
            .send(
                UMessageBuilder::publish(uri_provider.get_resource_uri(RESOURCE_ID))
                    .build()
                    .unwrap(),
            )
            .await
            .unwrap();

        let _ = std::fs::remove_file(SOCKET);
    }
}
