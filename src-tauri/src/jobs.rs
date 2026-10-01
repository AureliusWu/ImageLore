use crate::state::AppState;
use std::sync::{
    atomic::{AtomicBool, Ordering},
    Arc,
};

pub fn register(state: &AppState) -> Result<(u64, Arc<AtomicBool>), String> {
    let job_id = state.next_job_id.fetch_add(1, Ordering::Relaxed);
    let cancel = Arc::new(AtomicBool::new(false));
    state
        .jobs
        .lock()
        .map_err(|e| e.to_string())?
        .insert(job_id, cancel.clone());
    Ok((job_id, cancel))
}

pub fn cancel(state: &AppState, job_id: u64) -> Result<bool, String> {
    if let Some(flag) = state.jobs.lock().map_err(|e| e.to_string())?.get(&job_id) {
        flag.store(true, Ordering::Relaxed);
        return Ok(true);
    }
    Ok(false)
}

pub fn finish(state: &AppState, job_id: u64) {
    if let Ok(mut jobs) = state.jobs.lock() {
        jobs.remove(&job_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rusqlite::Connection;
    use std::{
        collections::HashMap,
        path::PathBuf,
        sync::{atomic::AtomicU64, Mutex},
    };

    fn state() -> AppState {
        AppState {
            db: Mutex::new(Connection::open_in_memory().unwrap()),
            backup_operation: Mutex::new(()),
            data_dir: PathBuf::new(),
            cache_dir: PathBuf::new(),
            database_path: PathBuf::new(),
            backups_dir: PathBuf::new(),
            models_dir: PathBuf::new(),
            jobs: Mutex::new(HashMap::new()),
            next_job_id: AtomicU64::new(1),
            vision_api_key: Mutex::new(String::new()),
        }
    }

    #[test]
    fn register_cancel_finish_round_trip() {
        let state = state();
        let (id, flag) = register(&state).unwrap();
        assert_eq!(id, 1);
        assert!(!flag.load(Ordering::Relaxed));
        assert!(cancel(&state, id).unwrap());
        assert!(flag.load(Ordering::Relaxed));
        finish(&state, id);
        assert!(!cancel(&state, id).unwrap());
    }
}
