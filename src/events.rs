//! Histórico de eventos por sessão e tempo em cada estado (ADR-0010).
//!
//! `state/events/<sid>.jsonl`, uma linha por evento de hook (ou ação do
//! ccdeck, ADR-0011). Passando de `ROTATE_BYTES`, o arquivo vira `.1.jsonl`
//! (o anterior é descartado).

use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::model::Status;
use crate::paths::Paths;

const ROTATE_BYTES: u64 = 512 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Event {
    pub at: DateTime<Utc>,
    pub status: Status,
    pub event: String,
}

fn file(paths: &Paths, sid: &str) -> PathBuf {
    paths.events_dir().join(format!("{sid}.jsonl"))
}

fn rotated(paths: &Paths, sid: &str) -> PathBuf {
    paths.events_dir().join(format!("{sid}.1.jsonl"))
}

/// Anexa um evento. Linhas pequenas com `O_APPEND`: hooks paralelos não se
/// atropelam.
pub fn append(paths: &Paths, sid: &str, ev: &Event) -> Result<()> {
    let path = file(paths, sid);
    std::fs::create_dir_all(paths.events_dir())?;
    if std::fs::metadata(&path).is_ok_and(|m| m.len() > ROTATE_BYTES) {
        let _ = std::fs::rename(&path, rotated(paths, sid));
    }
    let mut line = serde_json::to_vec(ev)?;
    line.push(b'\n');
    std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&path)?
        .write_all(&line)?;
    Ok(())
}

pub fn remove(paths: &Paths, sid: &str) {
    let _ = std::fs::remove_file(file(paths, sid));
    let _ = std::fs::remove_file(rotated(paths, sid));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grava_e_rotaciona() {
        let dir = std::env::temp_dir().join(format!("ccdeck-ev-{}", crate::model::short_id()));
        let paths = Paths::rooted(&dir);
        let ev = Event {
            at: Utc::now(),
            status: Status::Working,
            event: "PreToolUse · Bash".into(),
        };
        append(&paths, "s", &ev).unwrap();
        append(&paths, "s", &ev).unwrap();
        let txt = std::fs::read_to_string(file(&paths, "s")).unwrap();
        assert_eq!(txt.lines().count(), 2);
        std::fs::write(file(&paths, "s"), vec![b'x'; ROTATE_BYTES as usize + 1]).unwrap();
        append(&paths, "s", &ev).unwrap();
        assert!(rotated(&paths, "s").exists());
        remove(&paths, "s");
        assert!(!file(&paths, "s").exists() && !rotated(&paths, "s").exists());
        std::fs::remove_dir_all(dir).ok();
    }
}
