use bytes::Bytes;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

#[derive(Debug)]
pub struct DbHandle {
    db: Db,
}

#[derive(Debug, Clone)]
pub struct Db {
    shared: Arc<SharedState>,
}

#[derive(Debug)]
struct SharedState {
    state: Mutex<State>,
}

#[derive(Debug)]
struct State {
    entries: HashMap<String, Entry>,
}

#[derive(Debug)]
struct Entry {
    data: Bytes,
    expires_at: Option<Instant>,
}

impl Default for DbHandle {
    fn default() -> Self {
        Self::new()
    }
}

impl DbHandle {
    pub fn new() -> DbHandle {
        DbHandle { db: Db::new() }
    }

    pub fn db(&self) -> Db {
        self.db.clone()
    }
}

impl Default for Db {
    fn default() -> Self {
        Self::new()
    }
}

impl Db {
    pub fn new() -> Db {
        let shared = Arc::new(SharedState {
            state: Mutex::new(State {
                entries: HashMap::new(),
            }),
        });
        Db { shared }
    }

    pub fn get(&self, key: &str) -> Option<Bytes> {
        let state = self.shared.state.lock().unwrap();
        let value = match state.entries.get(key) {
            None => None,
            Some(e) => {
                let data = e.data.clone();
                match &e.expires_at {
                    None => Some(data),
                    Some(expiry) => {
                        if expiry < &Instant::now() {
                            // todo delete entry
                            None
                        } else {
                            Some(data)
                        }
                    }
                }
            }
        };
        value
    }

    pub fn set(&self, key: String, value: Bytes, duration: Option<Duration>) {
        let expires_at: Option<Instant> = duration.map(|d| Instant::now() + d);

        let mut state = self.shared.state.lock().unwrap();
        state.entries.insert(
            key,
            Entry {
                data: value,
                expires_at,
            },
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{thread, time};

    #[test]
    fn test_create_new_db() {
        let _ = Db::new();
    }

    #[test]
    fn test_set_and_get_key_in_db() {
        let db = Db::new();
        let key: &str = "Foo";
        let value = Bytes::from("Bar");
        db.set(key.to_string(), value.clone(), None);

        let value_got = db.get(key);

        assert_eq!(value, value_got.unwrap());
    }

    #[test]
    fn test_get_missing_entry_in_db() {
        let db = Db::new();
        let key: &str = "Foo";

        let value_got = db.get(key);

        assert!(value_got.is_none());
    }

    #[test]
    fn test_add_entry_to_db_with_expiry() {
        let db = Db::new();
        let key: &str = "Foo";
        let value = Bytes::from("Bar");
        let expiry = Duration::new(5, 0);

        db.set(key.to_string(), value.clone(), Some(expiry));

        let value_got = db.get(key);

        assert_eq!(value, value_got.unwrap());
    }

    #[test]
    fn test_get_entry_in_db_that_has_expired() {
        let db = Db::new();
        let key: &str = "Foo";
        let value = Bytes::from("Bar");
        let expiry = Duration::new(0, 10);

        db.set(key.to_string(), value, Some(expiry));

        let ten_millis = time::Duration::from_millis(10);
        thread::sleep(ten_millis);

        let value_got = db.get(key);

        assert!(value_got.is_none());
    }
}
