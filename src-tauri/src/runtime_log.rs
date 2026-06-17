use std::{
    fs::{create_dir_all, OpenOptions},
    io::Write,
    path::PathBuf,
    sync::{mpsc, OnceLock},
    thread,
};

use chrono::Utc;

const LOG_FILE_NAME: &str = "saynow-runtime.log";
static LOG_SENDER: OnceLock<Result<mpsc::Sender<String>, String>> = OnceLock::new();

pub fn write_line(message: &str) {
    match log_sender() {
        Ok(sender) => {
            if let Err(error) = sender.send(message.to_string()) {
                eprintln!("[saynow] failed to queue runtime log: {error}");
            }
        }
        Err(error) => eprintln!("[saynow] failed to initialize runtime log: {error}"),
    }
}

pub fn log_path_string() -> Result<String, String> {
    log_path().map(|path| path.display().to_string())
}

fn append_line(path: PathBuf, message: &str) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)
        .map_err(|error| format!("open {} failed: {error}", path.display()))?;
    let line = format!("{} {}\n", Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Millis, true), message);
    file.write_all(line.as_bytes())
        .map_err(|error| format!("write {} failed: {error}", path.display()))
}

fn log_sender() -> Result<&'static mpsc::Sender<String>, String> {
    LOG_SENDER
        .get_or_init(|| {
            let path = log_path()?;
            let (tx, rx) = mpsc::channel::<String>();
            thread::spawn(move || {
                for message in rx {
                    if let Err(error) = append_line(path.clone(), &message) {
                        eprintln!("[saynow] failed to write runtime log: {error}");
                    }
                }
            });
            Ok(tx)
        })
        .as_ref()
        .map_err(Clone::clone)
}

fn log_path() -> Result<PathBuf, String> {
    let base_dir = std::env::var_os("LOCALAPPDATA")
        .map(PathBuf::from)
        .or_else(|| std::env::var_os("APPDATA").map(PathBuf::from))
        .ok_or_else(|| "LOCALAPPDATA and APPDATA are not set".to_string())?;
    let log_dir = base_dir.join("saynow").join("logs");
    create_dir_all(&log_dir)
        .map_err(|error| format!("create {} failed: {error}", log_dir.display()))?;
    Ok(log_dir.join(LOG_FILE_NAME))
}
