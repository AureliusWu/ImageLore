use rusqlite::Connection;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{
        atomic::{AtomicBool,AtomicU64},
        Arc,Mutex,
    },
};

pub struct AppState {
    pub db: Mutex<Connection>,
    pub data_dir: PathBuf,
    pub cache_dir: PathBuf,
    pub database_path: PathBuf,
    pub backups_dir: PathBuf,
    pub import_jobs: Mutex<HashMap<u64,Arc<AtomicBool>>>,
    pub next_job_id: AtomicU64,
}
