use parking_lot::Mutex;
use std::collections::HashMap;
use std::time::Duration;
use tokio::time::sleep;

pub struct Presence {
    counts: Mutex<HashMap<u32, usize>>,
    debounce: Duration,
}

impl Presence {
    pub fn new(debounce: Duration) -> Self {
        Self {
            counts: Mutex::new(HashMap::new()),
            debounce,
        }
    }

    /// Register a connection. Returns true if user transitioned to online.
    pub fn connect(&self, user_id: u32) -> bool {
        let mut guard = self.counts.lock();
        let c = guard.entry(user_id).or_insert(0);
        *c += 1;
        *c == 1
    }

    /// Deregister a connection. Returns true if user transitions to offline after debounce.
    pub async fn disconnect(&self, user_id: u32) -> bool {
        {
            let mut guard = self.counts.lock();
            if let Some(c) = guard.get_mut(&user_id) {
                if *c > 0 {
                    *c -= 1;
                }
            }
        }
        sleep(self.debounce).await;
        let mut guard = self.counts.lock();
        match guard.get(&user_id).copied() {
            Some(0) | None => {
                guard.remove(&user_id);
                true
            }
            _ => false,
        }
    }

    pub fn snapshot(&self) -> HashMap<u32, &'static str> {
        let guard = self.counts.lock();
        guard
            .keys()
            .copied()
            .map(|id| (id, "online" as &'static str))
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::time::{sleep, Duration};

    #[tokio::test]
    async fn state_machine() {
        let presence = std::sync::Arc::new(Presence::new(Duration::from_millis(20)));
        assert!(presence.connect(1));
        let p = presence.clone();
        let fut = tokio::spawn(async move { p.disconnect(1).await });
        sleep(Duration::from_millis(10)).await;
        // reconnect before debounce expiry
        assert!(presence.connect(1));
        sleep(Duration::from_millis(30)).await;
        assert!(!fut.await.unwrap());
        // final disconnect
        assert!(presence.disconnect(1).await);
    }
}
