//! Histórico de eventos por sessão e tempo em cada estado (ADR-0010).
//!
//! `state/events/<sid>.jsonl`, uma linha por evento de hook (ou ação do
//! ccdeck, ADR-0011). Passando de `ROTATE_BYTES`, o arquivo vira `.1.jsonl`
//! (o anterior é descartado).

use std::collections::BTreeMap;
use std::io::Write;
use std::path::PathBuf;

use anyhow::Result;
use chrono::{DateTime, Duration, Utc};
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

/// Eventos em ordem de horário (inclui o arquivo rotacionado).
pub fn load(paths: &Paths, sid: &str) -> Vec<Event> {
    let mut out: Vec<Event> = [rotated(paths, sid), file(paths, sid)]
        .iter()
        .filter_map(|p| std::fs::read_to_string(p).ok())
        .flat_map(|s| {
            s.lines()
                .filter_map(|l| serde_json::from_str::<Event>(l).ok())
                .collect::<Vec<_>>()
        })
        .collect();
    out.sort_by_key(|e| e.at);
    out
}

pub fn remove(paths: &Paths, sid: &str) {
    let _ = std::fs::remove_file(file(paths, sid));
    let _ = std::fs::remove_file(rotated(paths, sid));
}

/// Tempo em cada estado dentro de `[from, to)`. Cada evento vale até o
/// próximo; o último vale até `end` (agora, se a sessão está viva; senão o
/// próprio horário dele, porque não sabemos quando o processo morreu).
pub fn time_in_states(
    events: &[Event],
    from: DateTime<Utc>,
    to: DateTime<Utc>,
    end: DateTime<Utc>,
) -> BTreeMap<Status, Duration> {
    let mut out: BTreeMap<Status, Duration> = BTreeMap::new();
    for (i, ev) in events.iter().enumerate() {
        let next = events.get(i + 1).map(|n| n.at).unwrap_or(end.max(ev.at));
        let start = ev.at.max(from);
        let stop = next.min(to);
        if stop > start {
            *out.entry(ev.status).or_insert_with(Duration::zero) += stop - start;
        }
    }
    out
}

/// Espera pelo usuário: *Aguardando aprovação* + *Aguardando você*.
pub fn waiting(times: &BTreeMap<Status, Duration>) -> Duration {
    [Status::Approval, Status::Yours]
        .iter()
        .filter_map(|s| times.get(s))
        .fold(Duration::zero(), |a, d| a + *d)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(min: i64) -> DateTime<Utc> {
        "2026-10-23T10:00:00Z".parse::<DateTime<Utc>>().unwrap() + Duration::minutes(min)
    }

    fn ev(min: i64, status: Status) -> Event {
        Event {
            at: at(min),
            status,
            event: String::new(),
        }
    }

    #[test]
    fn espera_somada() {
        // 7 min em aprovação, 22 min aguardando você (cenário da spec).
        let evs = [
            ev(0, Status::Working),
            ev(10, Status::Approval),
            ev(17, Status::Working),
            ev(30, Status::Yours),
            ev(52, Status::Working),
            ev(60, Status::Yours),
        ];
        let t = time_in_states(&evs, at(0), at(60), at(60));
        assert_eq!(t[&Status::Approval], Duration::minutes(7));
        assert_eq!(t[&Status::Yours], Duration::minutes(22));
        assert_eq!(waiting(&t), Duration::minutes(29));
        assert_eq!(t[&Status::Working], Duration::minutes(31));
    }

    #[test]
    fn recorta_periodo_e_fim() {
        let evs = [ev(0, Status::Yours), ev(30, Status::Working)];
        // Período começa no meio do primeiro intervalo.
        let t = time_in_states(&evs, at(20), at(100), at(40));
        assert_eq!(t[&Status::Yours], Duration::minutes(10));
        assert_eq!(t[&Status::Working], Duration::minutes(10));
        // Sessão morta: o último evento não acumula tempo.
        let t = time_in_states(&evs, at(0), at(100), at(30));
        assert!(!t.contains_key(&Status::Working));
    }

    #[test]
    fn grava_le_e_rotaciona() {
        let dir = std::env::temp_dir().join(format!("ccdeck-ev-{}", crate::model::short_id()));
        let paths = Paths::rooted(&dir);
        append(&paths, "s", &ev(1, Status::Working)).unwrap();
        append(&paths, "s", &ev(0, Status::Yours)).unwrap();
        let l = load(&paths, "s");
        assert_eq!(l.len(), 2);
        assert_eq!(l[0].status, Status::Yours, "ordenado por horário");
        // Força a rotação.
        let big = vec![b'x'; ROTATE_BYTES as usize + 1];
        std::fs::write(file(&paths, "s"), big).unwrap();
        append(&paths, "s", &ev(5, Status::Approval)).unwrap();
        assert!(rotated(&paths, "s").exists());
        assert_eq!(load(&paths, "s").len(), 1, "linhas inválidas ignoradas");
        remove(&paths, "s");
        assert!(load(&paths, "s").is_empty());
        std::fs::remove_dir_all(dir).ok();
    }
}
