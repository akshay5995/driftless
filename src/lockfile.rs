use crate::LOCKFILE_NAME;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

#[derive(Debug)]
pub(crate) enum LockfileError {
    Io(std::io::Error),
    Json(serde_json::Error),
}

impl std::fmt::Display for LockfileError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LockfileError::Io(err) => write!(f, "{err}"),
            LockfileError::Json(err) => write!(f, "{err}"),
        }
    }
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct Lockfile {
    #[serde(default)]
    pub(crate) refs: BTreeMap<String, String>,
}

pub(crate) fn lock_path(root: &Path) -> PathBuf {
    root.join(LOCKFILE_NAME)
}

pub(crate) fn load_lock(root: &Path) -> Result<Lockfile, LockfileError> {
    match std::fs::read_to_string(lock_path(root)) {
        Ok(s) => serde_json::from_str(&s).map_err(LockfileError::Json),
        Err(err) if err.kind() == std::io::ErrorKind::NotFound => Ok(Lockfile::default()),
        Err(err) => Err(LockfileError::Io(err)),
    }
}

pub(crate) fn save_lock(root: &Path, lock: &Lockfile) -> std::io::Result<()> {
    std::fs::write(
        lock_path(root),
        serde_json::to_string_pretty(lock).unwrap() + "\n",
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lockfile_round_trips_refs() {
        let dir = tempfile::tempdir().expect("create temp dir");
        let mut lock = Lockfile::default();
        lock.refs
            .insert("src/lib.rs#login".to_string(), "sig:body".to_string());

        save_lock(dir.path(), &lock).expect("save lock");
        let loaded = load_lock(dir.path()).expect("load lock");

        assert_eq!(loaded.refs["src/lib.rs#login"], "sig:body");
    }

    #[test]
    fn missing_lockfile_loads_as_empty() {
        let dir = tempfile::tempdir().expect("create temp dir");

        let loaded = load_lock(dir.path()).expect("load missing lock");

        assert!(loaded.refs.is_empty());
    }

    #[test]
    fn malformed_lockfile_is_an_error() {
        let dir = tempfile::tempdir().expect("create temp dir");
        std::fs::write(lock_path(dir.path()), "{not json").expect("write malformed lock");

        assert!(matches!(load_lock(dir.path()), Err(LockfileError::Json(_))));
    }
}
