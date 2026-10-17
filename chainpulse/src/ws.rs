use std::sync::Arc;

use axum::extract::State;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::response::{IntoResponse, Response};
use chainpulse_db::EventRecord;
use chainpulse_types::EventKind;
use serde::{Deserialize, Serialize};
use tokio::sync::OwnedSemaphorePermit;
use tokio::sync::broadcast::{self, error::RecvError};
use tokio_util::sync::CancellationToken;

use crate::api::{ApiError, AppState};
use crate::telemetry;

/// What indexers publish after each commit or rollback.
#[derive(Debug, Clone)]
pub enum Notification {
    Event(Arc<EventRecord>),
    Reorg(Reorg),
}

/// Every event above `common_ancestor` on this chain that was already streamed is gone.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Reorg {
    pub chain_id: u64,
    pub common_ancestor: u64,
    pub depth: u64,
    pub removed_events: u64,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Subscription {
    pub chain_id: Option<u64>,
    pub event_type: Option<EventKind>,
}

impl Subscription {
    /// Reorgs ignore `event_type`: a client needs every rollback on the chains it follows.
    fn wants(&self, notification: &Notification) -> bool {
        match notification {
            Notification::Event(event) => {
                self.chain_id
                    .is_none_or(|id| i64::try_from(id) == Ok(event.chain_id))
                    && self
                        .event_type
                        .is_none_or(|kind| kind.name() == event.event_type)
            }
            Notification::Reorg(reorg) => self.chain_id.is_none_or(|id| id == reorg.chain_id),
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct ClientMessage {
    subscribe: Subscription,
}

#[derive(Debug, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ServerMessage<'a> {
    Event(&'a EventRecord),
    Reorg(&'a Reorg),
    Subscribed(&'a Subscription),
    Lagged { skipped: u64 },
    Error { message: String },
}

impl ServerMessage<'_> {
    fn to_frame(&self) -> Message {
        match serde_json::to_string(self) {
            Ok(json) => Message::Text(json.into()),
            Err(error) => {
                tracing::error!(%error, "cannot serialize WebSocket message");
                Message::Text(r#"{"type":"error","message":"internal error"}"#.into())
            }
        }
    }
}

pub async fn upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    let Ok(permit) = state.ws_slots.clone().try_acquire_owned() else {
        return ApiError::Unavailable("too many WebSocket clients").into_response();
    };
    let feed = state.feed.subscribe();
    let shutdown = state.shutdown.clone();
    ws.on_upgrade(move |socket| stream(socket, feed, shutdown, permit))
}

async fn stream(
    mut socket: WebSocket,
    mut feed: broadcast::Receiver<Notification>,
    shutdown: CancellationToken,
    _permit: OwnedSemaphorePermit,
) {
    telemetry::ws_client_connected();
    let mut subscription = Subscription::default();
    loop {
        let frame = tokio::select! {
            () = shutdown.cancelled() => {
                let _ = socket.send(Message::Close(None)).await;
                break;
            }
            incoming = socket.recv() => match incoming {
                Some(Ok(Message::Text(text))) => match serde_json::from_str::<ClientMessage>(&text) {
                    Ok(message) => {
                        subscription = message.subscribe;
                        Some(ServerMessage::Subscribed(&subscription).to_frame())
                    }
                    Err(error) => Some(ServerMessage::Error { message: error.to_string() }.to_frame()),
                },
                Some(Ok(Message::Close(_)) | Err(_)) | None => break,
                Some(Ok(_)) => None,
            },
            notification = feed.recv() => match notification {
                Ok(notification) if subscription.wants(&notification) => Some(match &notification {
                    Notification::Event(event) => ServerMessage::Event(event).to_frame(),
                    Notification::Reorg(reorg) => ServerMessage::Reorg(reorg).to_frame(),
                }),
                Ok(_) => None,
                Err(RecvError::Lagged(skipped)) => Some(ServerMessage::Lagged { skipped }.to_frame()),
                Err(RecvError::Closed) => break,
            },
        };
        if let Some(frame) = frame
            && socket.send(frame).await.is_err()
        {
            break;
        }
    }
    telemetry::ws_client_disconnected();
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn event(chain_id: i64, event_type: &str) -> Notification {
        Notification::Event(Arc::new(EventRecord {
            id: 7,
            chain_id,
            block_number: 100,
            block_hash: "0x01".into(),
            block_timestamp: Default::default(),
            log_index: 0,
            tx_hash: "0x02".into(),
            contract_address: "0x03".into(),
            event_type: event_type.into(),
            payload: json!({}),
            confirmed: false,
        }))
    }

    fn reorg(chain_id: u64) -> Notification {
        Notification::Reorg(Reorg {
            chain_id,
            common_ancestor: 90,
            depth: 2,
            removed_events: 3,
        })
    }

    #[test]
    fn empty_subscription_receives_everything() {
        let all = Subscription::default();

        assert!(all.wants(&event(1, "Transfer")));
        assert!(all.wants(&reorg(137)));
    }

    #[test]
    fn filters_events_by_chain_and_type() {
        let sub = Subscription {
            chain_id: Some(1),
            event_type: Some(EventKind::Transfer),
        };

        assert!(sub.wants(&event(1, "Transfer")));
        assert!(!sub.wants(&event(1, "SwapV2")));
        assert!(!sub.wants(&event(137, "Transfer")));
    }

    #[test]
    fn reorgs_reach_every_subscriber_of_that_chain() {
        let sub = Subscription {
            chain_id: Some(1),
            event_type: Some(EventKind::Transfer),
        };

        assert!(sub.wants(&reorg(1)));
        assert!(!sub.wants(&reorg(137)));
    }

    #[test]
    fn messages_carry_a_type_tag() {
        let Notification::Reorg(r) = reorg(1) else {
            unreachable!()
        };
        let json: serde_json::Value =
            serde_json::from_str(&serde_json::to_string(&ServerMessage::Reorg(&r)).unwrap())
                .unwrap();

        assert_eq!(
            json,
            json!({"type": "reorg", "chain_id": 1, "common_ancestor": 90, "depth": 2, "removed_events": 3})
        );
    }

    #[test]
    fn rejects_unknown_event_types_in_subscriptions() {
        let err = serde_json::from_str::<ClientMessage>(r#"{"subscribe": {"event_type": "Mint"}}"#)
            .unwrap_err();

        assert!(err.to_string().contains("unknown event `Mint`"));
    }
}
