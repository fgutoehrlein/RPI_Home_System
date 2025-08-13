use std::collections::HashMap;
use std::time::{Duration, Instant};
use parking_lot::Mutex;
use uuid::Uuid;

pub struct TypingTracker {
    last: Mutex<HashMap<(u32, Uuid), Instant>>,
    debounce: Duration,
}

impl TypingTracker {
    pub fn new(debounce: Duration) -> Self {
        Self { last: Mutex::new(HashMap::new()), debounce }
    }

    /// Register a typing action. Returns true if event should be broadcast.
    pub fn typing(&self, user_id: u32, room_id: Uuid) -> bool {
        let mut guard = self.last.lock();
        let key = (user_id, room_id);
        let now = Instant::now();
        let should = match guard.get(&key) {
            Some(&prev) => now.duration_since(prev) >= self.debounce,
            None => true,
        };
        if should {
            guard.insert(key, now);
        }
        should
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn debounce_logic() {
        let tracker = TypingTracker::new(Duration::from_secs(2));
        let room = Uuid::nil();
        assert!(tracker.typing(1, room));
        assert!(!tracker.typing(1, room));
    }
}
