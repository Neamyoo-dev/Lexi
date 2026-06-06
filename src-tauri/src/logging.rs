use std::fs::{self, File, OpenOptions};
use std::io::Write;
use std::sync::Mutex;

pub struct LexiLogger {
    file: Mutex<Option<File>>,
}

impl LexiLogger {
    pub fn new() -> Self {
        let log_dir = std::env::var("LOCALAPPDATA")
            .map(|dir| std::path::Path::new(&dir).join("Lexi").join("logs"))
            .unwrap_or_else(|_| std::path::PathBuf::from("logs"));

        let _ = fs::create_dir_all(&log_dir);
        let log_path = log_dir.join("lexi.log");

        let file = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&log_path)
            .ok();

        LexiLogger {
            file: Mutex::new(file),
        }
    }
}

impl log::Log for LexiLogger {
    fn enabled(&self, _metadata: &log::Metadata) -> bool {
        true
    }

    fn log(&self, record: &log::Record) {
        let timestamp = chrono::Local::now().format("%Y-%m-%d %H:%M:%S%.3f");
        let msg = format!("{} [{}] {} - {}\n", timestamp, record.level(), record.target(), record.args());

        if let Ok(mut file_guard) = self.file.lock() {
            if let Some(ref mut file) = *file_guard {
                let _ = file.write_all(msg.as_bytes());
                let _ = file.flush();
            }
        }

        if cfg!(debug_assertions) {
            let _ = std::io::Write::write_all(&mut std::io::stderr(), msg.as_bytes());
        }
    }

    fn flush(&self) {
        if let Ok(mut file_guard) = self.file.lock() {
            if let Some(ref mut file) = *file_guard {
                let _ = file.flush();
            }
        }
    }
}

pub fn init_logging() {
    let logger = LexiLogger::new();
    log::set_boxed_logger(Box::new(logger)).ok();
    log::set_max_level(log::LevelFilter::Info);
}
