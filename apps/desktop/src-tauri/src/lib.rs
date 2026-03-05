pub mod store;
pub mod tracker;
pub mod types;

use store::SessionStore;
use tracker::{MacOSProbe, Tracker};
use types::TrackerConfig;

use chrono::{DateTime, Utc};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use tauri::{
    image::Image,
    menu::{Menu, MenuItem},
    tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
    Manager, RunEvent, WindowEvent,
};
use tauri_plugin_autostart::ManagerExt;

struct AppState {
    store: Arc<Mutex<SessionStore>>,
    icon_cache_dir: PathBuf,
    icon_cache: Mutex<HashMap<String, Option<String>>>,
}

#[tauri::command]
fn get_sessions(
    state: tauri::State<AppState>,
    start: String,
    end: String,
) -> Result<Vec<types::AppSession>, String> {
    let start: DateTime<Utc> = DateTime::parse_from_rfc3339(&start)
        .map_err(|e| e.to_string())?
        .with_timezone(&Utc);
    let end: DateTime<Utc> = DateTime::parse_from_rfc3339(&end)
        .map_err(|e| e.to_string())?
        .with_timezone(&Utc);

    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.get_sessions(start, end).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_daily_summary(
    state: tauri::State<AppState>,
    date: String,
) -> Result<types::DailySummary, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.get_daily_summary(&date).map_err(|e| e.to_string())
}

fn find_app_icon_path(bundle_id: &str) -> Option<PathBuf> {
    let output = std::process::Command::new("mdfind")
        .args(["kMDItemCFBundleIdentifier", "==", bundle_id])
        .output()
        .ok()?;
    let path_str = String::from_utf8_lossy(&output.stdout);
    let app_path = path_str.lines().next()?;
    let plist_path = format!("{}/Contents/Info.plist", app_path);
    let val = plist::Value::from_file(&plist_path).ok()?;
    let dict = val.as_dictionary()?;
    let icon_file = dict.get("CFBundleIconFile")?.as_string()?;
    let icon_name = if icon_file.ends_with(".icns") {
        icon_file.to_string()
    } else {
        format!("{}.icns", icon_file)
    };
    let full_path = PathBuf::from(format!("{}/Contents/Resources/{}", app_path, icon_name));
    if full_path.exists() {
        Some(full_path)
    } else {
        None
    }
}

#[tauri::command]
fn get_app_icon(
    state: tauri::State<AppState>,
    bundle_id: String,
) -> Result<Option<String>, String> {
    if let Ok(cache) = state.icon_cache.lock() {
        if let Some(cached) = cache.get(&bundle_id) {
            return Ok(cached.clone());
        }
    }

    let result = (|| -> Option<String> {
        let icns_path = find_app_icon_path(&bundle_id)?;
        let png_path = state.icon_cache_dir.join(format!("{}.png", bundle_id));

        if !png_path.exists() {
            let status = std::process::Command::new("sips")
                .args([
                    "-s",
                    "format",
                    "png",
                    "-z",
                    "32",
                    "32",
                    icns_path.to_str()?,
                    "--out",
                    png_path.to_str()?,
                ])
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .status()
                .ok()?;
            if !status.success() {
                return None;
            }
        }

        let bytes = std::fs::read(&png_path).ok()?;
        use base64::Engine;
        Some(base64::engine::general_purpose::STANDARD.encode(&bytes))
    })();

    if let Ok(mut cache) = state.icon_cache.lock() {
        cache.insert(bundle_id, result.clone());
    }

    Ok(result)
}

#[tauri::command]
fn check_accessibility() -> bool {
    macos_accessibility_check(false)
}

#[tauri::command]
fn request_accessibility() -> bool {
    macos_accessibility_check(true)
}

fn macos_accessibility_check(prompt: bool) -> bool {
    use std::ptr;
    extern "C" {
        fn AXIsProcessTrustedWithOptions(options: *const std::ffi::c_void) -> bool;
    }
    if !prompt {
        extern "C" {
            fn AXIsProcessTrusted() -> bool;
        }
        return unsafe { AXIsProcessTrusted() };
    }
    extern "C" {
        fn CFStringCreateWithCString(
            alloc: *const std::ffi::c_void,
            c_str: *const i8,
            encoding: u32,
        ) -> *const std::ffi::c_void;
        fn CFDictionaryCreate(
            allocator: *const std::ffi::c_void,
            keys: *const *const std::ffi::c_void,
            values: *const *const std::ffi::c_void,
            num_values: isize,
            key_callbacks: *const std::ffi::c_void,
            value_callbacks: *const std::ffi::c_void,
        ) -> *const std::ffi::c_void;
        fn CFRelease(cf: *const std::ffi::c_void);
        static kCFTypeDictionaryKeyCallBacks: std::ffi::c_void;
        static kCFTypeDictionaryValueCallBacks: std::ffi::c_void;
        static kCFBooleanTrue: *const std::ffi::c_void;
    }
    unsafe {
        let key = CFStringCreateWithCString(
            ptr::null(),
            c"AXTrustedCheckOptionPrompt".as_ptr(),
            0x08000100,
        );
        let keys = [key];
        let values = [kCFBooleanTrue];
        let options = CFDictionaryCreate(
            ptr::null(),
            keys.as_ptr(),
            values.as_ptr(),
            1,
            &kCFTypeDictionaryKeyCallBacks as *const _,
            &kCFTypeDictionaryValueCallBacks as *const _,
        );
        let trusted = AXIsProcessTrustedWithOptions(options);
        CFRelease(options);
        CFRelease(key);
        trusted
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_autostart::Builder::new().build())
        .setup(|app| {
            let app_data_dir = app.path().app_data_dir()?;
            std::fs::create_dir_all(&app_data_dir)?;
            let db_path = app_data_dir.join("record.db");

            let config = TrackerConfig::default();
            let conn = rusqlite::Connection::open(&db_path).expect("failed to open database");
            let store = Arc::new(Mutex::new(
                SessionStore::new(conn, config.clone()).expect("failed to initialize store"),
            ));

            let tracker = Tracker::new(store.clone(), config);
            tracker.start(MacOSProbe::new());

            let icon_cache_dir = app_data_dir.join("icon_cache");
            std::fs::create_dir_all(&icon_cache_dir)?;

            app.manage(AppState {
                store: store.clone(),
                icon_cache_dir,
                icon_cache: Mutex::new(HashMap::new()),
            });

            let show = MenuItem::with_id(app, "show", "Show Record", true, None::<&str>)?;
            let quit = MenuItem::with_id(app, "quit", "Quit", true, None::<&str>)?;
            let menu = Menu::with_items(app, &[&show, &quit])?;

            TrayIconBuilder::new()
                .icon(Image::from_bytes(include_bytes!("../icons/tray.png"))?)
                .icon_as_template(true)
                .menu(&menu)
                .tooltip("Record")
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => {
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.unminimize();
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                    "quit" => app.exit(0),
                    _ => {}
                })
                .on_tray_icon_event(|tray, event| {
                    if let TrayIconEvent::Click {
                        button: MouseButton::Left,
                        button_state: MouseButtonState::Up,
                        ..
                    } = event
                    {
                        let app = tray.app_handle();
                        if let Some(win) = app.get_webview_window("main") {
                            let _ = win.unminimize();
                            let _ = win.show();
                            let _ = win.set_focus();
                        }
                    }
                })
                .build(app)?;

            let autostart = app.autolaunch();
            if !autostart.is_enabled().unwrap_or(false) {
                let _ = autostart.enable();
            }

            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }

            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            get_sessions,
            get_daily_summary,
            get_app_icon,
            check_accessibility,
            request_accessibility
        ])
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(|_app, event| {
            if let RunEvent::ExitRequested { api, .. } = event {
                api.prevent_exit();
            }
        });
}
