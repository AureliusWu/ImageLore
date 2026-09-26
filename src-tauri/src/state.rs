use rusqlite::Connection;
use std::{path::PathBuf, sync::Mutex};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
}
