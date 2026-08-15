mod domain;
mod repository;
mod windows_session;

use std::{fs, sync::Mutex, thread, time::Duration};

use domain::{EventKind, Occurrence, WorkdaySummary};
use repository::Repository;
use serde::Serialize;
use tauri::{
    AppHandle, Emitter, Manager, State, WindowEvent,
    menu::{Menu, MenuItem, PredefinedMenuItem},
    tray::TrayIconBuilder,
};
use tauri_plugin_autostart::{MacosLauncher, ManagerExt};
use windows_session::WindowsSessionMonitor;

struct AppState {
    repository: Mutex<Repository>,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct Dashboard {
    today: Option<WorkdaySummary>,
    history: Vec<WorkdaySummary>,
    autostart_enabled: bool,
}

#[tauri::command]
fn get_dashboard(app: AppHandle, state: State<'_, AppState>) -> Result<Dashboard, String> {
    let now = Occurrence::now();
    let history = state
        .repository
        .lock()
        .map_err(|_| "Repository lock is poisoned".to_owned())?
        .list_workdays(now.utc_ms)
        .map_err(|error| error.to_string())?;
    let today = history
        .iter()
        .find(|workday| workday.date == now.local_date)
        .cloned();
    let autostart_enabled = app
        .autolaunch()
        .is_enabled()
        .map_err(|error| format!("Cannot read autostart setting: {error}"))?;

    Ok(Dashboard {
        today,
        history,
        autostart_enabled,
    })
}

#[tauri::command]
fn set_autostart_enabled(app: AppHandle, enabled: bool) -> Result<(), String> {
    let manager = app.autolaunch();
    if enabled {
        manager.enable()
    } else {
        manager.disable()
    }
    .map_err(|error| format!("Cannot update autostart setting: {error}"))
}

fn record_and_notify(app: &AppHandle, kind: EventKind) {
    let state = app.state::<AppState>();
    let result = state
        .repository
        .lock()
        .map_err(|_| "Repository lock is poisoned".to_owned())
        .and_then(|mut repository| {
            repository
                .record_event(kind, &Occurrence::now())
                .map_err(|error| error.to_string())
        });

    match result {
        Ok(()) => {
            let _ = app.emit("workday-updated", ());
        }
        Err(error) => eprintln!("Cannot record {}: {error}", kind.as_str()),
    }
}

fn start_projection_timer(app: AppHandle) {
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_secs(60));
            let state = app.state::<AppState>();
            let changed = state
                .repository
                .lock()
                .ok()
                .and_then(|mut repository| repository.advance_projection(&Occurrence::now()).ok())
                .unwrap_or(false);
            if changed {
                let _ = app.emit("workday-updated", ());
            }
        }
    });
}

fn build_tray(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Mở Workday Tracker", true, None::<&str>)?;
    let separator = PredefinedMenuItem::separator(app)?;
    let quit = MenuItem::with_id(app, "quit", "Thoát", true, None::<&str>)?;
    let menu = Menu::with_items(app, &[&open, &separator, &quit])?;

    let mut builder = TrayIconBuilder::with_id("main-tray")
        .menu(&menu)
        .tooltip("Workday Tracker")
        .on_menu_event(|app, event| match event.id().as_ref() {
            "open" => show_main_window(app),
            "quit" => app.exit(0),
            _ => {}
        })
        .on_tray_icon_event(|tray, event| {
            if let tauri::tray::TrayIconEvent::DoubleClick { .. } = event {
                show_main_window(tray.app_handle());
            }
        });

    if let Some(icon) = app.default_window_icon() {
        builder = builder.icon(icon.clone());
    }
    builder.build(app)?;
    Ok(())
}

fn show_main_window(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        // The documentation requires single-instance to be registered before other plugins.
        .plugin(tauri_plugin_single_instance::init(|app, _, _| {
            show_main_window(app)
        }))
        .plugin(tauri_plugin_autostart::init(
            MacosLauncher::LaunchAgent,
            Some(vec!["--background"]),
        ))
        .invoke_handler(tauri::generate_handler![
            get_dashboard,
            set_autostart_enabled
        ])
        .setup(|app| {
            let data_directory = app.path().app_local_data_dir()?;
            fs::create_dir_all(&data_directory)?;
            let database_path = data_directory.join("workday-tracker.db");
            let is_first_run = !database_path.exists();
            let repository = Repository::open(&database_path)?;
            app.manage(AppState {
                repository: Mutex::new(repository),
            });

            record_and_notify(app.handle(), EventKind::AppStarted);

            // Enable once during onboarding. Later launches must respect a user-disabled setting.
            let autostart = app.autolaunch();
            if is_first_run && !autostart.is_enabled().unwrap_or(false) {
                if let Err(error) = autostart.enable() {
                    eprintln!("Autostart was blocked by Windows policy: {error}");
                }
            }

            build_tray(app.handle())?;

            let event_app = app.handle().clone();
            let monitor = WindowsSessionMonitor::start(move |kind| {
                record_and_notify(&event_app, kind);
            })
            .map_err(std::io::Error::other)?;
            app.manage(monitor);

            start_projection_timer(app.handle().clone());

            if std::env::args().any(|argument| argument == "--background") {
                if let Some(window) = app.get_webview_window("main") {
                    window.hide()?;
                }
            }
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("error while running Workday Tracker");
}
