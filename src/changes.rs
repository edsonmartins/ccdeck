//! Varredura de alterações em segundo plano e colisões (ADR-0009).

use std::collections::{BTreeMap, HashMap};
use std::path::PathBuf;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use crate::git::{self, FileChange};
use crate::model::Session;

pub const SELECTED_EVERY: Duration = Duration::from_secs(3);
pub const ALL_EVERY: Duration = Duration::from_secs(15);

pub struct Job {
    pub session: String,
    pub cwd: PathBuf,
    pub base: Option<String>,
}

type Msg = (String, Result<Vec<FileChange>, String>);

pub struct Scanner {
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    in_flight: usize,
    pub last_all: Option<Instant>,
    pub last_selected: Option<Instant>,
    pub results: HashMap<String, Vec<FileChange>>,
    pub errors: HashMap<String, String>,
}

impl Default for Scanner {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            tx,
            rx,
            in_flight: 0,
            last_all: None,
            last_selected: None,
            results: HashMap::new(),
            errors: HashMap::new(),
        }
    }
}

impl Scanner {
    pub fn busy(&self) -> bool {
        self.in_flight > 0
    }

    /// Dispara uma thread que calcula as alterações dos jobs.
    pub fn spawn(&mut self, jobs: Vec<Job>) {
        if jobs.is_empty() {
            return;
        }
        self.in_flight += jobs.len();
        let tx = self.tx.clone();
        std::thread::spawn(move || {
            for j in jobs {
                let r = if j.cwd.is_dir() {
                    git::changes(&j.cwd, j.base.as_deref()).map_err(|e| format!("{e:#}"))
                } else {
                    Err("diretório não existe".into())
                };
                if tx.send((j.session, r)).is_err() {
                    return;
                }
            }
        });
    }

    /// Recolhe resultados prontos. Retorna true se algo mudou.
    pub fn collect(&mut self) -> bool {
        let mut changed = false;
        while let Ok((sid, r)) = self.rx.try_recv() {
            self.in_flight = self.in_flight.saturating_sub(1);
            match r {
                Ok(v) => {
                    self.errors.remove(&sid);
                    if self.results.get(&sid) != Some(&v) {
                        self.results.insert(sid, v);
                        changed = true;
                    }
                }
                Err(e) => {
                    self.errors.insert(sid, e);
                }
            }
        }
        changed
    }
}

/// Sessão → (arquivo → outras sessões que também o alteraram).
pub fn collisions(
    sessions: &[Session],
    results: &HashMap<String, Vec<FileChange>>,
) -> HashMap<String, BTreeMap<String, Vec<String>>> {
    let mut by_file: HashMap<(&str, &str), Vec<&str>> = HashMap::new();
    for s in sessions {
        for c in results.get(&s.id).map(Vec::as_slice).unwrap_or_default() {
            by_file
                .entry((s.project_id.as_str(), c.path.as_str()))
                .or_default()
                .push(s.id.as_str());
        }
    }
    let mut out: HashMap<String, BTreeMap<String, Vec<String>>> = HashMap::new();
    for ((_, file), ids) in by_file.into_iter().filter(|(_, ids)| ids.len() > 1) {
        for id in &ids {
            let others = ids
                .iter()
                .filter(|o| *o != id)
                .map(|o| o.to_string())
                .collect();
            out.entry(id.to_string())
                .or_default()
                .insert(file.to_string(), others);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::git::ChangeKind;

    fn sess(id: &str, project: &str) -> Session {
        Session {
            id: id.into(),
            project_id: project.into(),
            name: id.into(),
            cwd: "/tmp".into(),
            branch: None,
            worktree: true,
            claude_session_id: uuid::Uuid::new_v4(),
            permission_mode: "default".into(),
            notes: String::new(),
            created_at: chrono::Utc::now(),
            started_once: true,
            base: Some("main".into()),
            port_slot: None,
            ports: vec![],
        }
    }

    fn ch(path: &str) -> FileChange {
        FileChange {
            path: path.into(),
            added: Some(1),
            deleted: Some(0),
            kind: ChangeKind::Modified,
        }
    }

    #[test]
    fn detecta_colisao_so_no_mesmo_projeto() {
        let sessions = vec![sess("a", "p1"), sess("b", "p1"), sess("c", "p2")];
        let mut r = HashMap::new();
        r.insert("a".to_string(), vec![ch("pom.xml"), ch("A.java")]);
        r.insert("b".to_string(), vec![ch("pom.xml")]);
        r.insert("c".to_string(), vec![ch("pom.xml")]);
        let col = collisions(&sessions, &r);
        assert_eq!(col["a"]["pom.xml"], vec!["b"]);
        assert_eq!(col["b"]["pom.xml"], vec!["a"]);
        assert!(!col.contains_key("c"));
        assert!(!col["a"].contains_key("A.java"));
    }
}
