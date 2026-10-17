#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::time::Duration;
use tauri::{webview::PageLoadEvent, AppHandle, Manager};

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

fn main() {
    tauri::Builder::default()
        // As soon as famlyfi.com has finished loading in the (hidden) main window, swap it in.
        .on_page_load(|webview, payload| {
            if webview.label() == "main" && matches!(payload.event(), PageLoadEvent::Finished) {
                reveal(webview.app_handle());
            }
        })
        // Safety net: never leave the user stuck on the splash.
        .setup(|app| {
            let handle = app.handle().clone();
            std::thread::spawn(move || {
                std::thread::sleep(Duration::from_secs(15));
                let h = handle.clone();
                let _ = handle.run_on_main_thread(move || reveal(&h));
            });
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running Famlyfi");
}
