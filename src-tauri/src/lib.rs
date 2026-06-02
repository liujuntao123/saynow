pub mod commands;
pub mod db;
pub mod models;
pub mod platform;
pub mod prompt;
pub mod provider;
pub mod stats;

#[cfg(feature = "desktop")]
pub fn run() {
    let db_path = std::env::current_dir()
        .expect("failed to read current dir")
        .join("voice-input-assistant.sqlite");
    let db = db::AppDb::open(&db_path).expect("failed to open app database");

    tauri::Builder::default()
        .manage(db)
        .invoke_handler(commands::handlers())
        .run(tauri::generate_context!())
        .expect("failed to run tauri application");
}

#[cfg(not(feature = "desktop"))]
pub fn run() {
    eprintln!("desktop feature is disabled; skipping Tauri runtime startup");
}
