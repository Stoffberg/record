pub mod project;
pub mod store;
pub mod tracker;
pub mod types;

use project::ProjectDetector;
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
    tz_offset_minutes: i32,
) -> Result<types::DailySummary, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .get_daily_summary(&date, tz_offset_minutes)
        .map_err(|e| e.to_string())
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
fn get_app_sessions(
    state: tauri::State<AppState>,
    date: String,
    bundle_id: String,
    tz_offset_minutes: i32,
) -> Result<Vec<types::AppSession>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .get_app_sessions(&date, &bundle_id, tz_offset_minutes)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_app_averages(
    state: tauri::State<AppState>,
    date: String,
    bundle_id: String,
    tz_offset_minutes: i32,
) -> Result<(f64, f64), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .get_app_averages(&bundle_id, &date, tz_offset_minutes)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn add_exclusion(
    state: tauri::State<AppState>,
    bundle_id: String,
    app_name: String,
    expires_at: Option<String>,
) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .add_exclusion(&bundle_id, &app_name, expires_at.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_exclusion(state: tauri::State<AppState>, bundle_id: String) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .remove_exclusion(&bundle_id)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_exclusions(
    state: tauri::State<AppState>,
) -> Result<Vec<(String, String, Option<String>)>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.get_exclusions().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_app_projects(
    state: tauri::State<AppState>,
    date: String,
    bundle_id: String,
    tz_offset_minutes: i32,
) -> Result<Vec<types::ProjectUsage>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .get_app_projects(&date, &bundle_id, tz_offset_minutes)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_daily_projects(
    state: tauri::State<AppState>,
    date: String,
    tz_offset_minutes: i32,
) -> Result<Vec<types::ProjectUsage>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .get_daily_projects(&date, tz_offset_minutes)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn create_space(
    state: tauri::State<AppState>,
    name: String,
    color: String,
    initials: String,
    emoji: Option<String>,
) -> Result<types::Space, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .create_space(&name, &color, &initials, emoji.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn update_space(
    state: tauri::State<AppState>,
    id: i64,
    name: String,
    color: String,
    initials: String,
    emoji: Option<String>,
) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .update_space(id, &name, &color, &initials, emoji.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn delete_space(state: tauri::State<AppState>, id: i64) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.delete_space(id).map_err(|e| e.to_string())
}

#[tauri::command]
fn get_spaces(state: tauri::State<AppState>) -> Result<Vec<types::SpaceWithProjects>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.get_spaces().map_err(|e| e.to_string())
}

#[tauri::command]
fn add_project_to_space(
    state: tauri::State<AppState>,
    space_id: i64,
    project: String,
) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .add_project_to_space(space_id, &project)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_project_from_space(
    state: tauri::State<AppState>,
    space_id: i64,
    project: String,
) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .remove_project_from_space(space_id, &project)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_all_projects(state: tauri::State<AppState>) -> Result<Vec<(String, i64)>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.get_all_projects().map_err(|e| e.to_string())
}

#[tauri::command]
fn add_project_exclusion(
    state: tauri::State<AppState>,
    project: String,
    expires_at: Option<String>,
) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .add_project_exclusion(&project, expires_at.as_deref())
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn remove_project_exclusion(state: tauri::State<AppState>, project: String) -> Result<(), String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .remove_project_exclusion(&project)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn get_project_exclusions(
    state: tauri::State<AppState>,
) -> Result<Vec<(String, Option<String>)>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store.get_project_exclusions().map_err(|e| e.to_string())
}

#[tauri::command]
fn get_daily_spaces(
    state: tauri::State<AppState>,
    date: String,
    tz_offset_minutes: i32,
) -> Result<Vec<types::SpaceUsage>, String> {
    let store = state.store.lock().map_err(|e| e.to_string())?;
    store
        .get_daily_spaces(&date, tz_offset_minutes)
        .map_err(|e| e.to_string())
}

#[tauri::command]
fn backfill_projects(state: tauri::State<AppState>) -> Result<u64, String> {
    let detector = ProjectDetector::new();
    let store = state.store.lock().map_err(|e| e.to_string())?;

    let mut stmt = store
        .conn()
        .prepare(
            "SELECT id, bundle_id, window_title, started_at
             FROM app_sessions WHERE is_idle = 0",
        )
        .map_err(|e| e.to_string())?;

    let rows: Vec<(i64, String, String, String)> = stmt
        .query_map([], |row| {
            Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?))
        })
        .map_err(|e| e.to_string())?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|e| e.to_string())?;

    let mut updated = 0u64;
    for (id, bundle_id, window_title, started_at_str) in &rows {
        let timestamp = chrono::DateTime::parse_from_rfc3339(started_at_str)
            .map(|dt| dt.with_timezone(&chrono::Utc))
            .unwrap_or_else(|_| chrono::Utc::now());

        match detector.detect_at(bundle_id, window_title, timestamp) {
            Some(ctx) => {
                store
                    .conn()
                    .execute(
                        "UPDATE app_sessions SET project = ?1, detail = ?2 WHERE id = ?3",
                        rusqlite::params![ctx.project, ctx.detail, id],
                    )
                    .map_err(|e| e.to_string())?;
                updated += 1;
            }
            None => {
                store
                    .conn()
                    .execute(
                        "UPDATE app_sessions SET project = NULL, detail = NULL WHERE id = ?1 AND project IS NOT NULL",
                        rusqlite::params![id],
                    )
                    .map_err(|e| e.to_string())?;
            }
        }
    }

    Ok(updated)
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

            let tracker = Tracker::new(store.clone(), ProjectDetector::new(), config);
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
            get_app_sessions,
            get_app_averages,
            add_exclusion,
            remove_exclusion,
            get_exclusions,
            get_app_projects,
            get_daily_projects,
            create_space,
            update_space,
            delete_space,
            get_spaces,
            add_project_to_space,
            remove_project_from_space,
            get_all_projects,
            add_project_exclusion,
            remove_project_exclusion,
            get_project_exclusions,
            get_daily_spaces,
            backfill_projects,
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
