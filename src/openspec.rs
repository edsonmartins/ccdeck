//! Changes do OpenSpec de um repositório (ADR-0014).

use std::path::Path;

/// Prompt padrão para uma sessão a partir de um change; `{id}` e `{dir}`.
pub const DEFAULT_PROMPT: &str = "Implemente o change OpenSpec {id}. Leia {dir}/proposal.md, \
{dir}/tasks.md e {dir}/specs/ e siga as tarefas na ordem, marcando cada uma como concluída \
no tasks.md ao terminar.";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Change {
    pub id: String,
    /// Caminho relativo à raiz do repositório.
    pub dir: String,
    pub title: String,
    pub done: usize,
    pub total: usize,
}

impl Change {
    pub fn finished(&self) -> bool {
        self.total > 0 && self.done == self.total
    }

    pub fn prompt(&self, template: Option<&str>) -> String {
        template
            .unwrap_or(DEFAULT_PROMPT)
            .replace("{id}", &self.id)
            .replace("{dir}", &self.dir)
    }
}

/// `openspec/changes/<id>/` com `proposal.md`, menos `archive/`. Pendentes
/// primeiro, depois por id.
pub fn list(repo: &Path) -> Vec<Change> {
    let base = repo.join("openspec").join("changes");
    let Ok(entries) = std::fs::read_dir(&base) else {
        return Vec::new();
    };
    let mut out: Vec<Change> = entries
        .flatten()
        .filter(|e| e.path().is_dir())
        .filter_map(|e| {
            let id = e.file_name().to_string_lossy().to_string();
            if id == "archive" || id.starts_with('.') {
                return None;
            }
            let proposal = std::fs::read_to_string(e.path().join("proposal.md")).ok()?;
            let title = proposal
                .lines()
                .find_map(|l| l.strip_prefix("# "))
                .map(|t| t.trim_start_matches("Change:").trim().to_string())
                .unwrap_or_else(|| id.clone());
            let tasks = std::fs::read_to_string(e.path().join("tasks.md")).unwrap_or_default();
            let (done, total) = progress(&tasks);
            Some(Change {
                dir: format!("openspec/changes/{id}"),
                id,
                title,
                done,
                total,
            })
        })
        .collect();
    out.sort_by(|a, b| (a.finished(), &a.id).cmp(&(b.finished(), &b.id)));
    out
}

/// (marcadas, total) das linhas `- [x]` / `- [ ]`.
pub fn progress(tasks: &str) -> (usize, usize) {
    tasks
        .lines()
        .map(str::trim_start)
        .fold((0, 0), |(d, t), l| {
            if l.starts_with("- [x]") || l.starts_with("- [X]") {
                (d + 1, t + 1)
            } else if l.starts_with("- [ ]") {
                (d, t + 1)
            } else {
                (d, t)
            }
        })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lista_changes_com_progresso() {
        let dir = std::env::temp_dir().join(format!("ccdeck-os-{}", crate::model::short_id()));
        let mk = |id: &str, proposal: &str, tasks: &str| {
            let d = dir.join("openspec/changes").join(id);
            std::fs::create_dir_all(&d).unwrap();
            std::fs::write(d.join("proposal.md"), proposal).unwrap();
            std::fs::write(d.join("tasks.md"), tasks).unwrap();
        };
        mk(
            "add-b",
            "# Change: faz B\n",
            "- [x] 1\n  - [ ] 2\n- [ ] 3\n",
        );
        mk("add-a", "# Change: faz A\n", "- [x] 1\n- [X] 2\n");
        mk("archive", "# x\n", "");
        std::fs::create_dir_all(dir.join("openspec/changes/sem-proposta")).unwrap();
        let l = list(&dir);
        let ids: Vec<&str> = l.iter().map(|c| c.id.as_str()).collect();
        assert_eq!(
            ids,
            ["add-b", "add-a"],
            "pendente primeiro; archive e sem proposta fora"
        );
        assert_eq!(
            (l[0].title.as_str(), l[0].done, l[0].total),
            ("faz B", 1, 3)
        );
        assert!(l[1].finished());
        let p = l[0].prompt(None);
        assert!(p.contains("openspec/changes/add-b/tasks.md"), "{p}");
        assert_eq!(
            l[0].prompt(Some("faça {id} em {dir}")),
            "faça add-b em openspec/changes/add-b"
        );
        assert!(list(&dir.join("nada")).is_empty());
        std::fs::remove_dir_all(dir).ok();
    }
}
