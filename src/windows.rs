//! Janelas auxiliares da sessão (ADR-0015).

use std::collections::BTreeMap;

use crate::tmux::{CLAUDE_WINDOW, SETUP_WINDOW, VERIFY_WINDOW};

pub const SHELL_WINDOW: &str = "shell";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    Claude,
    Shell,
    /// Definida em `windows` no `.ccdeck.json`.
    Project,
    /// Já existe na sessão (setup, verify ou criada à mão).
    Other,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WinItem {
    pub name: String,
    pub kind: Kind,
    /// Comando para criar a janela; `None` = não dá para recriar.
    pub command: Option<String>,
    pub open: bool,
}

/// Lista do modal: claude, shell, as do projeto e as demais que existem.
/// Retorna também os nomes reservados ignorados do `.ccdeck.json`.
pub fn items(
    existing: &[String],
    configured: &BTreeMap<String, String>,
    shell: &str,
) -> (Vec<WinItem>, Vec<String>) {
    let open = |n: &str| existing.iter().any(|e| e == n);
    let mut out = vec![
        WinItem {
            name: CLAUDE_WINDOW.into(),
            kind: Kind::Claude,
            command: None,
            open: open(CLAUDE_WINDOW),
        },
        WinItem {
            name: SHELL_WINDOW.into(),
            kind: Kind::Shell,
            command: Some(shell.to_string()),
            open: open(SHELL_WINDOW),
        },
    ];
    let reserved = [CLAUDE_WINDOW, SETUP_WINDOW, VERIFY_WINDOW, SHELL_WINDOW];
    let mut ignored = Vec::new();
    for (name, cmd) in configured {
        if reserved.contains(&name.as_str()) || name.trim().is_empty() || name.contains(':') {
            ignored.push(name.clone());
            continue;
        }
        out.push(WinItem {
            name: name.clone(),
            kind: Kind::Project,
            command: Some(cmd.clone()),
            open: open(name),
        });
    }
    for e in existing {
        if !out.iter().any(|i| &i.name == e) {
            out.push(WinItem {
                name: e.clone(),
                kind: Kind::Other,
                command: None,
                open: true,
            });
        }
    }
    (out, ignored)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn monta_lista() {
        let existing = vec!["claude".to_string(), "verify".into(), "servidor".into()];
        let cfg: BTreeMap<String, String> = [
            ("servidor".to_string(), "npm run dev".to_string()),
            ("logs".into(), "tail -f app.log".into()),
            ("claude".into(), "rm -rf /".into()),
        ]
        .into();
        let (items, ignored) = items(&existing, &cfg, "/bin/zsh");
        let names: Vec<(&str, bool)> = items.iter().map(|i| (i.name.as_str(), i.open)).collect();
        assert_eq!(
            names,
            [
                ("claude", true),
                ("shell", false),
                ("logs", false),
                ("servidor", true),
                ("verify", true)
            ]
        );
        assert_eq!(ignored, ["claude"]);
        assert_eq!(items[1].command.as_deref(), Some("/bin/zsh"));
        assert_eq!(items[4].kind, Kind::Other);
    }
}
