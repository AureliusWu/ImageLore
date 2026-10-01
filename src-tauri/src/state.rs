use rusqlite::Connection;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool, AtomicU64},
        Arc, Mutex,
    },
};

pub struct AppState {
    pub db: Mutex<Connection>,
    // Backup operations acquire this before db. Validation and rotation must
    // not occupy the connection needed by normal library reads and edits.
    pub backup_operation: Mutex<()>,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub database_path: PathBuf,
    pub backups_dir: PathBuf,
    pub models_dir: PathBuf,
    pub jobs: Mutex<HashMap<u64, Arc<AtomicBool>>>,
    pub next_job_id: AtomicU64,
    pub vision_api_key: Mutex<String>,
}
