use crate::project::ProjectDetector;
use crate::store::SessionStore;
use crate::types::{Heartbeat, TrackerConfig};
use chrono::Utc;
use log::{error, info, warn};
use std::collections::HashMap;
use std::path::Path;
use std::sync::mpsc;
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

fn ax_window_title(pid: u64) -> Option<String> {
    use std::ffi::c_void;
    use std::ptr;

    type AXUIElementRef = *const c_void;
    type AXError = i32;
    type CFTypeRef = *const c_void;
    type CFStringRef = *const c_void;

    const K_AX_ERROR_SUCCESS: AXError = 0;

    extern "C" {
        fn AXUIElementCreateApplication(pid: i32) -> AXUIElementRef;
        fn AXUIElementCopyAttributeValue(
            element: AXUIElementRef,
            attribute: CFStringRef,
            value: *mut CFTypeRef,
        ) -> AXError;
        fn CFStringCreateWithCString(
            alloc: *const c_void,
            c_str: *const i8,
            encoding: u32,
        ) -> CFStringRef;
        fn CFGetTypeID(cf: CFTypeRef) -> u64;
        fn CFStringGetTypeID() -> u64;
        fn CFStringGetLength(string: CFStringRef) -> isize;
        fn CFStringGetCString(
            string: CFStringRef,
            buffer: *mut i8,
            buffer_size: isize,
            encoding: u32,
        ) -> bool;
        fn CFRelease(cf: CFTypeRef);
    }

    const K_CF_STRING_ENCODING_UTF8: u32 = 0x08000100;

    unsafe {
        let app = AXUIElementCreateApplication(pid as i32);
        if app.is_null() {
            return None;
        }

        let focused_window_attr = CFStringCreateWithCString(
            ptr::null(),
            c"AXFocusedWindow".as_ptr(),
            K_CF_STRING_ENCODING_UTF8,
        );

        let mut focused_window: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(app, focused_window_attr, &mut focused_window);
        CFRelease(focused_window_attr);

        if err != K_AX_ERROR_SUCCESS || focused_window.is_null() {
            CFRelease(app);
            return None;
        }

        let title_attr =
            CFStringCreateWithCString(ptr::null(), c"AXTitle".as_ptr(), K_CF_STRING_ENCODING_UTF8);

        let mut title_value: CFTypeRef = ptr::null();
        let err = AXUIElementCopyAttributeValue(focused_window, title_attr, &mut title_value);
        CFRelease(title_attr);
        CFRelease(focused_window);
        CFRelease(app);

        if err != K_AX_ERROR_SUCCESS || title_value.is_null() {
            return None;
        }

        if CFGetTypeID(title_value) != CFStringGetTypeID() {
            CFRelease(title_value);
            return None;
        }

        let len = CFStringGetLength(title_value);
        let buf_size = len * 4 + 1;
        let mut buf = vec![0i8; buf_size as usize];
        let ok = CFStringGetCString(
            title_value,
            buf.as_mut_ptr(),
            buf_size,
            K_CF_STRING_ENCODING_UTF8,
        );
        CFRelease(title_value);

        if !ok {
            return None;
        }

        let cstr = std::ffi::CStr::from_ptr(buf.as_ptr());
        let title = cstr.to_string_lossy().into_owned();
        if title.is_empty() {
            None
        } else {
            Some(title)
        }
    }
}

impl SystemProbe for MacOSProbe {
    fn foreground_app(&self) -> Option<(String, String, String)> {
        match active_win_pos_rs::get_active_window() {
            Ok(win) => {
                let app_name = win.app_name;
                let bundle_id = self.cached_bundle_id(&win.process_path);
                let title = if win.title.is_empty() {
                    ax_window_title(win.process_id).unwrap_or_default()
                } else {
                    win.title
                };
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

#[cfg(not(test))]
#[allow(clippy::upper_case_acronyms)]
mod observer {
    use log::{info, warn};
    use std::ffi::c_void;
    use std::ptr;
    use std::sync::mpsc;
    use std::thread;

    type Id = *mut c_void;
    type SEL = *mut c_void;

    #[repr(C)]
    struct BlockLiteral {
        isa: *const c_void,
        flags: i32,
        reserved: i32,
        invoke: extern "C" fn(*mut BlockLiteral, Id),
        descriptor: *const BlockDescriptor,
        tx: *const mpsc::Sender<()>,
    }

    #[repr(C)]
    struct BlockDescriptor {
        reserved: u64,
        size: u64,
    }

    static BLOCK_DESCRIPTOR: BlockDescriptor = BlockDescriptor {
        reserved: 0,
        size: std::mem::size_of::<BlockLiteral>() as u64,
    };

    extern "C" fn block_invoke(block: *mut BlockLiteral, _notification: Id) {
        let tx = unsafe { &*(*block).tx };
        let _ = tx.send(());
    }

    extern "C" {
        static _NSConcreteGlobalBlock: *const c_void;
        fn objc_getClass(name: *const i8) -> Id;
        fn sel_registerName(name: *const i8) -> SEL;
    }

    unsafe fn msg_send_0(receiver: Id, sel: SEL) -> Id {
        type MsgSend = unsafe extern "C" fn(Id, SEL) -> Id;
        let f: MsgSend = std::mem::transmute(objc_msgSend as *const c_void);
        f(receiver, sel)
    }

    unsafe fn msg_send_4(receiver: Id, sel: SEL, a: Id, b: Id, c: Id, d: Id) -> Id {
        type MsgSend = unsafe extern "C" fn(Id, SEL, Id, Id, Id, Id) -> Id;
        let f: MsgSend = std::mem::transmute(objc_msgSend as *const c_void);
        f(receiver, sel, a, b, c, d)
    }

    extern "C" {
        fn objc_msgSend();
        fn CFRunLoopRun();
    }

    pub fn spawn(tx: mpsc::Sender<()>) {
        thread::spawn(move || unsafe {
            let tx_leaked: &'static mpsc::Sender<()> = Box::leak(Box::new(tx));

            let block = Box::leak(Box::new(BlockLiteral {
                isa: &_NSConcreteGlobalBlock as *const _ as *const c_void,
                flags: (1 << 28),
                reserved: 0,
                invoke: block_invoke,
                descriptor: &BLOCK_DESCRIPTOR,
                tx: tx_leaked as *const mpsc::Sender<()>,
            }));

            let workspace_cls = objc_getClass(c"NSWorkspace".as_ptr());
            let workspace =
                msg_send_0(workspace_cls, sel_registerName(c"sharedWorkspace".as_ptr()));
            if workspace.is_null() {
                warn!("Failed to get NSWorkspace.sharedWorkspace");
                return;
            }

            let center = msg_send_0(workspace, sel_registerName(c"notificationCenter".as_ptr()));
            if center.is_null() {
                warn!("Failed to get NSWorkspace notification center");
                return;
            }

            let name_cls = objc_getClass(c"NSString".as_ptr());
            let name_alloc = msg_send_0(name_cls, sel_registerName(c"alloc".as_ptr()));

            type InitWithUtf8 = unsafe extern "C" fn(Id, SEL, *const i8) -> Id;
            let init_sel = sel_registerName(c"initWithUTF8String:".as_ptr());
            let init_fn: InitWithUtf8 = std::mem::transmute(objc_msgSend as *const c_void);
            let notif_name = init_fn(
                name_alloc,
                init_sel,
                c"NSWorkspaceDidActivateApplicationNotification".as_ptr(),
            );

            let add_sel = sel_registerName(c"addObserverForName:object:queue:usingBlock:".as_ptr());
            let block_ptr = block as *mut BlockLiteral as Id;
            msg_send_4(
                center,
                add_sel,
                notif_name,
                ptr::null_mut(),
                ptr::null_mut(),
                block_ptr,
            );

            info!("App switch observer started (NSWorkspace notification)");

            CFRunLoopRun();
        });
    }
}

pub struct Tracker {
    store: Arc<Mutex<SessionStore>>,
    detector: ProjectDetector,
    config: TrackerConfig,
}

impl Tracker {
    pub fn new(
        store: Arc<Mutex<SessionStore>>,
        detector: ProjectDetector,
        config: TrackerConfig,
    ) -> Self {
        Self {
            store,
            detector,
            config,
        }
    }

    fn record_heartbeat<P: SystemProbe>(&self, probe: &P) {
        if let Some((app_name, bundle_id, window_title)) = probe.foreground_app() {
            let excluded = self
                .store
                .lock()
                .ok()
                .and_then(|s| s.is_excluded(&bundle_id).ok())
                .unwrap_or(false);

            if !excluded {
                let is_idle = probe.idle_seconds() >= self.config.idle_threshold_secs;
                let now = Utc::now();
                let ctx = self.detector.detect_at(&bundle_id, &window_title, now);
                let heartbeat = Heartbeat {
                    app_name,
                    bundle_id,
                    window_title,
                    is_idle,
                    timestamp: now,
                    project: ctx.as_ref().map(|c| c.project.clone()),
                    detail: ctx.and_then(|c| c.detail),
                };

                if let Ok(store) = self.store.lock() {
                    if let Err(e) = store.record_heartbeat(heartbeat) {
                        error!("Failed to record heartbeat: {}", e);
                    }
                }
            }
        }
    }

    pub fn start<P: SystemProbe>(self, probe: P) -> thread::JoinHandle<()> {
        let interval = Duration::from_secs(self.config.poll_interval_secs);

        let (tx, rx) = mpsc::channel::<()>();

        #[cfg(not(test))]
        observer::spawn(tx);

        #[cfg(test)]
        let _ = tx;

        thread::spawn(move || {
            info!(
                "Tracker started, polling every {}s with event-driven app switch detection",
                interval.as_secs()
            );
            loop {
                self.record_heartbeat(&probe);

                match rx.recv_timeout(interval) {
                    Ok(()) => {}
                    Err(mpsc::RecvTimeoutError::Timeout) => {}
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        warn!("App switch observer disconnected, falling back to polling only");
                        loop {
                            thread::sleep(interval);
                            self.record_heartbeat(&probe);
                        }
                    }
                }
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

        let tracker = Tracker::new(store.clone(), ProjectDetector::new(), config);
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
