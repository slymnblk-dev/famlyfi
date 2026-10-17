#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;
use tauri::{webview::PageLoadEvent, AppHandle, Manager};
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_updater::UpdaterExt;

/// Show the main window and close the splash (safe to call more than once).
fn reveal(app: &AppHandle) {
    if let Some(main) = app.get_webview_window("main") {
        let _ = main.show();
        let _ = main.set_focus();
    }
    if let Some(splash) = app.get_webview_window("splash") {
        let _ = splash.close();
    }
}

/// Look for a newer desktop version. If there is one, download it quietly,
/// then ask the user whether to restart now or later.
async fn check_for_update(app: AppHandle) -> Result<(), tauri_plugin_updater::Error> {
    let Some(update) = app.updater()?.check().await? else {
        return Ok(());
    };
    let bytes = update.download(|_, _| {}, || {}).await?;
    let version = update.version.clone();
    let handle = app.clone();
    app.dialog()
        .message(format!(
            "A new version of Famlyfi ({version}) is ready.\nRestart now to finish updating?"
        ))
        .title("Update ready")
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCancelCustom(
            "Restart now".into(),
            "Later".into(),
        ))
        .show(move |restart| {
            if restart && update.install(bytes).is_ok() {
                handle.restart();
            }
        });
    Ok(())
}

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        // As soon as famlyfi.com has finished loading in the (hidden) main window, swap it in.
        .on_page_load(|webview, payload| {
            if webview.label() == "main" && matches!(payload.event(), PageLoadEvent::Finished) {
                reveal(webview.app_handle());
            }
        })
        .setup(|app| {
            // Safety net: never leave the user stuck on the splash.
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(15));
                let h = handle.clone();
                let _ = handle.run_on_main_thread(move || reveal(&h));
            });
            // Check for desktop updates in the background.
            let handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                let _ = check_for_update(handle).await;
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Famlyfi");
}
