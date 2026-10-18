#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Mutex,
    },
    time::Duration,
};
use tauri::{AppHandle, Manager, State};
use tauri_plugin_updater::{Update, UpdaterExt};

/// The update found by the last check, waiting for the user to install it.
#[derive(Default)]
struct PendingUpdate(Mutex<Option<Update>>);

#[derive(serde::Serialize)]
struct UpdateInfo {
    version: String,
    current: String,
    notes: Option<String>,
}

/// Called by the app (on start and from Settings > Check for updates).
#[tauri::command]
async fn check_update(
    app: AppHandle,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<UpdateInfo>, String> {
    let update = app
        .updater()
        .map_err(|e| e.to_string())?
        .check()
        .await
        .map_err(|e| e.to_string())?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current: u.current_version.clone(),
        notes: u.body.clone(),
    });
    *pending.0.lock().unwrap() = update;
    Ok(info)
}

/// Called when the user clicks "Update & restart".
#[tauri::command]
async fn install_update(app: AppHandle, pending: State<'_, PendingUpdate>) -> Result<(), String> {
    let update = pending.0.lock().unwrap().take();
    let Some(update) = update else {
        return Err("No update is ready".into());
    };
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}

/// Start-up state: is the app drawn, and has the main window been revealed yet?
#[derive(Default)]
struct Boot {
    app_ready: AtomicBool,
    revealed: AtomicBool,
}

/// Called by the app once its first screen is drawn. Returns true if the window is already visible
/// (e.g. after an in-app refresh), so the app can skip its fade-in.
#[tauri::command]
fn app_ready(boot: State<'_, Boot>) -> bool {
    boot.app_ready.store(true, Ordering::SeqCst);
    boot.revealed.load(Ordering::SeqCst)
}

/// Asked by the splash: may it start fading out?
#[tauri::command]
fn app_is_ready(boot: State<'_, Boot>) -> bool {
    boot.app_ready.load(Ordering::SeqCst)
}

/// Called by the splash after its fade-out finished.
#[tauri::command]
fn splash_done(app: AppHandle) {
    reveal(&app);
}

/// Show the main window and close the splash (safe to call more than once).
fn reveal(app: &AppHandle) {
    if app.state::<Boot>().revealed.swap(true, Ordering::SeqCst) {
        return;
    }
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
        // Let the app fade in now that it is on screen.
        let _ = main.eval("document.documentElement.classList.add('shown')");
    }
    if let Some(splash) = app.get_webview_window("splash") {
        let _ = splash.close();
    }
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(PendingUpdate::default())
        .manage(Boot::default())
        .invoke_handler(tauri::generate_handler![
            check_update,
            install_update,
            app_ready,
            app_is_ready,
            splash_done
        ])
        .setup(|app| {
            // Safety net: never leave the user stuck on the splash.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(12));
                let h = handle.clone();
                let _ = handle.run_on_main_thread(move || reveal(&h));
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Famlyfi");
}
