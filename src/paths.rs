use std::path::PathBuf;

pub fn state_dir() -> Option<PathBuf> {
    if let Some(explicit) = std::env::var_os("UPM_STATE_DIR") {
        return Some(PathBuf::from(explicit));
    }
    dirs::state_dir()
        .or_else(dirs::config_dir)
        .map(|dir| dir.join("upm"))
}

pub fn profiles_dir() -> Option<PathBuf> {
    state_dir().map(|dir| dir.join("profiles"))
}

pub fn snapshots_dir() -> Option<PathBuf> {
    state_dir().map(|dir| dir.join("snapshots"))
}

pub fn history_path() -> Option<PathBuf> {
    state_dir().map(|dir| dir.join("history.jsonl"))
}
