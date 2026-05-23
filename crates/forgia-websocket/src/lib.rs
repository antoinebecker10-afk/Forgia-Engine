//! forgia-websocket — Bevy WebSocket client plugin (tungstenite-based).
//!
//! Inspired by Renzora's `websocket_plugin` but native (no Renzora editor deps).
//!
//! Pattern : spawn one OS thread per connection that bridges tungstenite ↔ crossbeam channels
//! ↔ Bevy ECS events. Bevy systems remain single-threaded compatible.
//!
//! Usage :
//! ```ignore
//! app.add_plugins(ForgiaWebsocketPlugin);
//! // In a system :
//! ws_open.send(WsOpenRequest { url: "ws://localhost:8080".into() });
//! ```

use bevy::prelude::*;
use crossbeam_channel::{Receiver, Sender};
use std::collections::HashMap;
use std::thread;

/// Request to open a new WS connection.
#[derive(Message, Clone)]
pub struct WsOpenRequest {
    pub id: WsConnectionId,
    pub url: String,
}

/// Request to send a text message on an existing connection.
#[derive(Message, Clone)]
pub struct WsSendRequest {
    pub id: WsConnectionId,
    pub payload: String,
}

/// Inbound message from a WS connection.
#[derive(Message, Clone)]
pub struct WsMessage {
    pub id: WsConnectionId,
    pub payload: String,
}

/// Connection lifecycle events.
#[derive(Message, Clone)]
pub enum WsLifecycle {
    Connected(WsConnectionId),
    Disconnected(WsConnectionId, String),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct WsConnectionId(pub u64);

#[derive(Resource, Default)]
struct WsConnections {
    handles: HashMap<WsConnectionId, ConnectionHandle>,
}

struct ConnectionHandle {
    tx_out: Sender<String>,     // game -> ws thread
    rx_in: Receiver<WsInbound>, // ws thread -> game
}

enum WsInbound {
    Connected,
    Message(String),
    Disconnected(String),
}

pub struct ForgiaWebsocketPlugin;

impl Plugin for ForgiaWebsocketPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<WsConnections>()
            .add_message::<WsOpenRequest>()
            .add_message::<WsSendRequest>()
            .add_message::<WsMessage>()
            .add_message::<WsLifecycle>()
            .add_systems(
                Update,
                (handle_open_requests, handle_send_requests, pump_inbound),
            );
    }
}

fn handle_open_requests(
    mut events: MessageReader<WsOpenRequest>,
    mut conns: ResMut<WsConnections>,
) {
    for ev in events.read() {
        let (tx_out, rx_out) = crossbeam_channel::unbounded::<String>();
        let (tx_in, rx_in) = crossbeam_channel::unbounded::<WsInbound>();
        let id = ev.id;
        let url = ev.url.clone();
        thread::spawn(move || ws_worker(id, url, rx_out, tx_in));
        conns.handles.insert(id, ConnectionHandle { tx_out, rx_in });
    }
}

fn handle_send_requests(mut events: MessageReader<WsSendRequest>, conns: Res<WsConnections>) {
    for ev in events.read() {
        if let Some(h) = conns.handles.get(&ev.id) {
            let _ = h.tx_out.send(ev.payload.clone());
        }
    }
}

fn pump_inbound(
    conns: Res<WsConnections>,
    mut msg_w: MessageWriter<WsMessage>,
    mut life_w: MessageWriter<WsLifecycle>,
) {
    let ids: Vec<WsConnectionId> = conns.handles.keys().copied().collect();
    for id in ids {
        if let Some(h) = conns.handles.get(&id) {
            while let Ok(msg) = h.rx_in.try_recv() {
                match msg {
                    WsInbound::Connected => {
                        life_w.write(WsLifecycle::Connected(id));
                    }
                    WsInbound::Message(p) => {
                        msg_w.write(WsMessage { id, payload: p });
                    }
                    WsInbound::Disconnected(reason) => {
                        life_w.write(WsLifecycle::Disconnected(id, reason));
                    }
                }
            }
        }
    }
}

fn ws_worker(_id: WsConnectionId, url: String, rx_out: Receiver<String>, tx_in: Sender<WsInbound>) {
    let Ok(parsed) = url::Url::parse(&url) else {
        let _ = tx_in.send(WsInbound::Disconnected(format!("invalid url: {url}")));
        return;
    };

    let (mut socket, _resp) = match tungstenite::connect(parsed.as_str()) {
        Ok(p) => p,
        Err(e) => {
            let _ = tx_in.send(WsInbound::Disconnected(format!("connect failed: {e}")));
            return;
        }
    };

    let _ = tx_in.send(WsInbound::Connected);

    loop {
        // Drain pending outbound
        while let Ok(out) = rx_out.try_recv() {
            if socket.send(tungstenite::Message::Text(out)).is_err() {
                let _ = tx_in.send(WsInbound::Disconnected("send error".into()));
                return;
            }
        }

        match socket.read() {
            Ok(tungstenite::Message::Text(t)) => {
                let _ = tx_in.send(WsInbound::Message(t.to_string()));
            }
            Ok(tungstenite::Message::Close(_)) => {
                let _ = tx_in.send(WsInbound::Disconnected("remote closed".into()));
                return;
            }
            Ok(_) => {} // ping/pong/binary ignored for V1
            Err(e) => {
                let _ = tx_in.send(WsInbound::Disconnected(format!("read error: {e}")));
                return;
            }
        }
    }
}
