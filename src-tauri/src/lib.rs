pub mod commands;
pub mod db;
pub mod learning;
pub mod models;
pub mod platform;
pub mod prompt;
pub mod provider;
pub mod stats;

#[cfg(feature = "desktop")]
use tauri::Manager;

#[cfg(feature = "desktop")]
pub fn run() {
    use tauri::{
        menu::MenuBuilder,
        tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent},
        WindowEvent,
    };

    tauri::Builder::default()
        .invoke_handler(commands::handlers())
        .setup(|app| {
            let db = open_app_database(app)?;
            app.manage(db);

            #[cfg(target_os = "windows")]
            if let Some(window) = app.get_webview_window("main") {
                allow_microphone_permission(&window)?;
            }

            let recorder_window = create_recorder_window(app)?;
            #[cfg(target_os = "windows")]
            allow_microphone_permission(&recorder_window)?;

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
fn open_app_database<R: tauri::Runtime>(app: &tauri::App<R>) -> Result<db::AppDb, tauri::Error> {
    let data_dir = app.path().app_data_dir().map_err(|error| {
        tauri::Error::Anyhow(anyhow::anyhow!(
            "failed to resolve app data directory: {error}"
        ))
    })?;
    std::fs::create_dir_all(&data_dir).map_err(|error| {
        tauri::Error::Anyhow(anyhow::anyhow!(
            "failed to create app data directory {}: {error}",
            data_dir.display()
        ))
    })?;

    let db_path = data_dir.join("saynow.sqlite");
    migrate_legacy_database(&db_path)?;
    db::AppDb::open(&db_path).map_err(|error| {
        tauri::Error::Anyhow(anyhow::anyhow!(
            "failed to open app database {}: {error}",
            db_path.display()
        ))
    })
}

#[cfg(feature = "desktop")]
fn migrate_legacy_database(db_path: &std::path::Path) -> Result<(), tauri::Error> {
    if db_path.exists() {
        return Ok(());
    }

    let legacy_path = std::env::current_dir()
        .map_err(|error| {
            tauri::Error::Anyhow(anyhow::anyhow!("failed to read current directory: {error}"))
        })?
        .join("saynow.sqlite");
    migrate_legacy_database_from(&legacy_path, db_path)
}

#[cfg(feature = "desktop")]
fn migrate_legacy_database_from(
    legacy_path: &std::path::Path,
    db_path: &std::path::Path,
) -> Result<(), tauri::Error> {
    if legacy_path == db_path || db_path.exists() || !legacy_path.exists() {
        return Ok(());
    }

    std::fs::copy(&legacy_path, db_path).map_err(|error| {
        tauri::Error::Anyhow(anyhow::anyhow!(
            "failed to migrate app database from {} to {}: {error}",
            legacy_path.display(),
            db_path.display()
        ))
    })?;
    Ok(())
}

#[cfg(all(test, feature = "desktop"))]
mod tests {
    use super::*;

    #[test]
    fn migrates_legacy_database_when_app_data_database_is_missing() {
        let test_dir = std::env::temp_dir().join(format!(
            "saynow-db-migration-test-{}-{}",
            std::process::id(),
            chrono::Utc::now().timestamp_nanos_opt().unwrap_or_default()
        ));
        let legacy_dir = test_dir.join("legacy");
        let data_dir = test_dir.join("data");
        std::fs::create_dir_all(&legacy_dir).unwrap();
        std::fs::create_dir_all(&data_dir).unwrap();

        let legacy_path = legacy_dir.join("saynow.sqlite");
        let db_path = data_dir.join("saynow.sqlite");
        let legacy_db = db::AppDb::open(&legacy_path).unwrap();
        legacy_db
            .save_config(&db::AppConfig {
                provider: "Qwen".to_string(),
                base_url: "https://dashscope.aliyuncs.com/compatible-mode/v1".to_string(),
                model: "qwen-custom".to_string(),
                api_key_ref: "literal:test-key".to_string(),
                hotkey: "Alt+Space".to_string(),
            })
            .unwrap();
        drop(legacy_db);

        migrate_legacy_database_from(&legacy_path, &db_path).unwrap();

        let migrated_db = db::AppDb::open(&db_path).unwrap();
        let config = migrated_db.get_config().unwrap();
        assert_eq!(config.provider, "Qwen");
        assert_eq!(config.model, "qwen-custom");
        assert_eq!(config.api_key_ref, "literal:test-key");
        assert_eq!(config.hotkey, "Alt+Space");

        let _ = std::fs::remove_dir_all(test_dir);
    }
}

#[cfg(feature = "desktop")]
fn create_recorder_window<R: tauri::Runtime>(
    app: &tauri::App<R>,
) -> Result<tauri::WebviewWindow<R>, tauri::Error> {
    let window = tauri::WebviewWindowBuilder::new(
        app,
        "recorder",
        tauri::WebviewUrl::App("index.html?view=recorder".into()),
    )
    .title("说文录音")
    .inner_size(760.0, 52.0)
    .resizable(false)
    .maximizable(false)
    .minimizable(false)
    .closable(false)
    .decorations(false)
    .always_on_top(true)
    .skip_taskbar(true)
    .visible(false)
    .focused(false)
    .focusable(true)
    .transparent(true)
    .shadow(false)
    .build()?;

    #[cfg(target_os = "windows")]
    {
        let hwnd = window.hwnd()?;
        crate::platform::configure_no_activate_window(hwnd.0 as isize)
            .map_err(|error| tauri::Error::Anyhow(anyhow::anyhow!(error)))?;
    }

    Ok(window)
}

#[cfg(all(feature = "desktop", target_os = "windows"))]
fn allow_microphone_permission<R: tauri::Runtime>(
    window: &tauri::WebviewWindow<R>,
) -> Result<(), tauri::Error> {
    window.with_webview(|webview| {
        use webview2_com::{
            Microsoft::Web::WebView2::Win32::{
                COREWEBVIEW2_PERMISSION_KIND, COREWEBVIEW2_PERMISSION_KIND_MICROPHONE,
                COREWEBVIEW2_PERMISSION_STATE_ALLOW,
            },
            PermissionRequestedEventHandler,
        };

        unsafe {
            let webview = match webview.controller().CoreWebView2() {
                Ok(webview) => webview,
                Err(error) => {
                    eprintln!("[saynow] failed to get WebView2 for microphone permission: {error}");
                    return;
                }
            };

            let mut token = 0;
            if let Err(error) = webview.add_PermissionRequested(
                &PermissionRequestedEventHandler::create(Box::new(|_, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };

                    let mut kind = COREWEBVIEW2_PERMISSION_KIND::default();
                    args.PermissionKind(&mut kind)?;
                    if kind == COREWEBVIEW2_PERMISSION_KIND_MICROPHONE {
                        args.SetState(COREWEBVIEW2_PERMISSION_STATE_ALLOW)?;
                    }

                    Ok(())
                })),
                &mut token,
            ) {
                eprintln!(
                    "[saynow] failed to install WebView2 microphone permission handler: {error}"
                );
            }
        }
    })?;

    Ok(())
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
