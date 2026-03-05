use crate::store::SessionStore;
use crate::types::{Heartbeat, TrackerConfig};
use chrono::Utc;
use log::{error, info};
use std::collections::HashMap;
use std::path::Path;
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::Duration;

pub trait SystemProbe: Send + 'static {
    fn foreground_app(&self) -> Option<(String, String, String)>;
    fn idle_seconds(&self) -> u64;
}

fn resolve_bundle_id(process_path: &Path) -> Option<String> {
    let mut path = process_path.to_path_buf();
    loop {
        if path.extension().and_then(|e| e.to_str()) == Some("app") {
            break;
        }
        if !path.pop() {
            return None;
        }
    }
    let plist_path = path.join("Contents/Info.plist");
    let val = plist::Value::from_file(&plist_path).ok()?;
    let dict = val.as_dictionary()?;
    dict.get("CFBundleIdentifier")?
        .as_string()
        .map(|s| s.to_string())
}

pub struct MacOSProbe {
    cache: Mutex<HashMap<String, String>>,
}

impl Default for MacOSProbe {
    fn default() -> Self {
        Self {
            cache: Mutex::new(HashMap::new()),
        }
    }
}

impl MacOSProbe {
    pub fn new() -> Self {
        Self::default()
    }

    fn cached_bundle_id(&self, process_path: &Path) -> String {
        let key = process_path.to_string_lossy().to_string();
        if let Ok(cache) = self.cache.lock() {
            if let Some(id) = cache.get(&key) {
                return id.clone();
            }
        }

        let bundle_id = resolve_bundle_id(process_path).unwrap_or_else(|| key.clone());

        if let Ok(mut cache) = self.cache.lock() {
            cache.insert(key, bundle_id.clone());
        }

        bundle_id
    }
}

impl SystemProbe for MacOSProbe {
    fn foreground_app(&self) -> Option<(String, String, String)> {
        match active_win_pos_rs::get_active_window() {
            Ok(win) => {
                let app_name = win.app_name;
                let bundle_id = self.cached_bundle_id(&win.process_path);
                let title = win.title;
                Some((app_name, bundle_id, title))
            }
            Err(_) => None,
        }
    }

    fn idle_seconds(&self) -> u64 {
        user_idle::UserIdle::get_time()
            .map(|t| t.as_seconds())
            .unwrap_or(0)
    }
}

pub struct Tracker {
    store: Arc<Mutex<SessionStore>>,
    config: TrackerConfig,
}

impl Tracker {
    pub fn new(store: Arc<Mutex<SessionStore>>, config: TrackerConfig) -> Self {
        Self { store, config }
    }

    pub fn start<P: SystemProbe>(self, probe: P) -> thread::JoinHandle<()> {
        let interval = Duration::from_secs(self.config.poll_interval_secs);
        let idle_threshold = self.config.idle_threshold_secs;

        thread::spawn(move || {
            info!("Tracker started, polling every {}s", interval.as_secs());
            loop {
                if let Some((app_name, bundle_id, window_title)) = probe.foreground_app() {
                    let is_idle = probe.idle_seconds() >= idle_threshold;
                    let heartbeat = Heartbeat {
                        app_name,
                        bundle_id,
                        window_title,
                        is_idle,
                        timestamp: Utc::now(),
                    };

                    if let Ok(store) = self.store.lock() {
                        if let Err(e) = store.record_heartbeat(heartbeat) {
                            error!("Failed to record heartbeat: {}", e);
                        }
                    }
                }

                thread::sleep(interval);
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::TrackerConfig;
    use rusqlite::Connection;
    use std::sync::atomic::{AtomicU32, Ordering};

    struct FakeProbe {
        call_count: Arc<AtomicU32>,
        idle_secs: u64,
    }

    impl SystemProbe for FakeProbe {
        fn foreground_app(&self) -> Option<(String, String, String)> {
            self.call_count.fetch_add(1, Ordering::SeqCst);
            Some((
                "Safari".to_string(),
                "com.apple.Safari".to_string(),
                "Google".to_string(),
            ))
        }

        fn idle_seconds(&self) -> u64 {
            self.idle_secs
        }
    }

    #[test]
    fn resolve_bundle_id_from_app_path() {
        let safari = std::path::Path::new("/Applications/Safari.app");
        let result = super::resolve_bundle_id(safari);
        assert_eq!(result, Some("com.apple.Safari".to_string()));
    }

    #[test]
    fn resolve_bundle_id_from_nested_binary_path() {
        let path = std::path::Path::new("/Applications/Safari.app/Contents/MacOS/Safari");
        let result = super::resolve_bundle_id(path);
        assert_eq!(result, Some("com.apple.Safari".to_string()));
    }

    #[test]
    fn resolve_bundle_id_returns_none_for_invalid_path() {
        let path = std::path::Path::new("/usr/bin/ls");
        let result = super::resolve_bundle_id(path);
        assert!(result.is_none());
    }

    #[test]
    fn tracker_records_heartbeats_via_probe() {
        let conn = Connection::open_in_memory().unwrap();
        let config = TrackerConfig {
            poll_interval_secs: 1,
            idle_threshold_secs: 300,
            merge_gap_secs: 10,
        };
        let store = Arc::new(Mutex::new(SessionStore::new(conn, config.clone()).unwrap()));
        let call_count = Arc::new(AtomicU32::new(0));

        let probe = FakeProbe {
            call_count: call_count.clone(),
            idle_secs: 0,
        };

        let tracker = Tracker::new(store.clone(), config);
        let handle = tracker.start(probe);

        thread::sleep(Duration::from_millis(2500));

        assert!(
            call_count.load(Ordering::SeqCst) >= 2,
            "probe should be called at least twice"
        );

        let store = store.lock().unwrap();
        let now = Utc::now();
        let sessions = store
            .get_sessions(
                now - chrono::Duration::hours(1),
                now + chrono::Duration::hours(1),
            )
            .unwrap();

        assert_eq!(
            sessions.len(),
            1,
            "heartbeats should merge into one session"
        );
        assert_eq!(sessions[0].app_name, "Safari");
        assert!(!sessions[0].is_idle);

        drop(store);
        drop(handle);
    }
}
