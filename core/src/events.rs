use std::collections::HashMap;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver, UnboundedSender};

/// Very small event bus used internally by the core.  It is intentionally
/// minimal and only supports broadcasting string payloads.
pub struct EventBus {
    subscribers: HashMap<String, Vec<UnboundedSender<String>>>,
}

impl EventBus {
    pub fn new() -> Self {
        Self {
            subscribers: HashMap::new(),
        }
    }

    /// Subscribe to a topic, returning a receiver for events.
    pub fn subscribe(&mut self, topic: &str) -> UnboundedReceiver<String> {
        let (tx, rx) = unbounded_channel();
        self.subscribers
            .entry(topic.to_string())
            .or_default()
            .push(tx);
        rx
    }

    /// Publish a message on a topic.
    pub fn publish(&mut self, topic: &str, payload: String) {
        if let Some(list) = self.subscribers.get_mut(topic) {
            list.retain(|tx| tx.send(payload.clone()).is_ok());
        }
    }
}

impl Default for EventBus {
    fn default() -> Self {
        Self::new()
    }
}
