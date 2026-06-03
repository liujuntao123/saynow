pub mod commands;
pub mod db;
pub mod models;
pub mod platform;
pub mod prompt;
pub mod provider;
pub mod stats;

#[cfg(feature = "desktop")]
pub fn run() {
    use tauri::{
        menu::MenuBuilder,
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
        Manager, WindowEvent,
    };

    let db_path = std::env::current_dir()
        .expect("failed to read current dir")
        .join("saynow.sqlite");
    let db = db::AppDb::open(&db_path).expect("failed to open app database");

    tauri::Builder::default()
        .manage(db)
        .invoke_handler(commands::handlers())
        .setup(|app| {
            let menu = MenuBuilder::new(app)
                .text("show", "显示主窗口")
                .text("quit", "退出")
                .build()?;

            let mut tray = TrayIconBuilder::new()
                .menu(&menu)
                .tooltip("说文")
                .show_menu_on_left_click(false)
                .on_menu_event(|app, event| match event.id().as_ref() {
                    "show" => show_main_window(app),
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
                        show_main_window(tray.app_handle());
                    }
                });

            if let Some(icon) = app.default_window_icon().cloned() {
                tray = tray.icon(icon);
            }

            tray.build(app)?;
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                api.prevent_close();
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}

#[cfg(feature = "desktop")]
fn show_main_window<R: tauri::Runtime>(app: &tauri::AppHandle<R>) {
    use tauri::Manager;

    if let Some(window) = app.get_webview_window("main") {
        let _ = window.show();
        let _ = window.set_focus();
    }
}

#[cfg(not(feature = "desktop"))]
pub fn run() {
    eprintln!("desktop feature is disabled; skipping Tauri runtime startup");
}
