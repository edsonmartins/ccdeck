//! Modelo de domínio: projetos, sessões, estados e configuração.

use std::path::PathBuf;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

pub const SCHEMA_VERSION: u32 = 2;

/// Estado de uma sessão, derivado de hooks (ADR-0004) e da presença no tmux.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Starting,
    Working,
    Approval,
    Yours,
    Error,
    Ended,
    Suspended,
}

impl Status {
    pub fn label(self) -> &'static str {
        match self {
            Status::Starting => "Iniciando",
            Status::Working => "Trabalhando",
            Status::Approval => "Aguardando aprovação",
            Status::Yours => "Aguardando você",
            Status::Error => "Erro",
            Status::Ended => "Encerrada",
            Status::Suspended => "Suspensa — Enter retoma",
        }
    }

    pub fn short(self) -> &'static str {
        match self {
            Status::Starting => "inic.",
            Status::Working => "trab.",
            Status::Approval => "aprov.",
            Status::Yours => "sua vez",
            Status::Error => "erro",
            Status::Ended => "fim",
            Status::Suspended => "susp.",
        }
    }

    pub fn icon(self) -> &'static str {
        match self {
            Status::Starting => "○",
            Status::Working => "●",
            Status::Approval => "▲",
            Status::Yours => "◆",
            Status::Error => "✕",
            Status::Ended => "■",
            Status::Suspended => "‖",
        }
    }

    /// Estados que colocam a sessão na fila "Atenção" e geram notificação.
    pub fn needs_attention(self) -> bool {
        matches!(self, Status::Approval | Status::Yours | Status::Error)
    }

    /// Ordem de urgência na visão Atenção (menor = mais urgente).
    pub fn urgency(self) -> u8 {
        match self {
            Status::Approval => 0,
            Status::Error => 1,
            Status::Yours => 2,
            _ => 9,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Project {
    pub id: String,
    pub name: String,
    pub path: PathBuf,
    #[serde(default)]
    pub collapsed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Session {
    /// Identificador curto do ccdeck; também nome da sessão tmux e `CCDECK_SID`.
    pub id: String,
    pub project_id: String,
    pub name: String,
    pub cwd: PathBuf,
    pub branch: Option<String>,
    /// Se `cwd` é um worktree criado pelo ccdeck.
    pub worktree: bool,
    pub claude_session_id: Uuid,
    pub permission_mode: String,
    #[serde(default)]
    pub notes: String,
    pub created_at: DateTime<Utc>,
    /// true depois do primeiro start: a partir daí só se retoma com --resume.
    #[serde(default)]
    pub started_once: bool,
    /// Branch base do worktree (para calcular alterações). `None` = HEAD.
    #[serde(default)]
    pub base: Option<String>,
    /// Slot de portas no projeto (ADR-0008). 0/None = sem portas.
    #[serde(default)]
    pub port_slot: Option<u32>,
    #[serde(default)]
    pub ports: Vec<PortVar>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PortVar {
    pub name: String,
    pub port: u16,
}

impl Session {
    pub fn tmux_name(&self) -> String {
        format!("cd-{}", self.id)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct State {
    pub schema_version: u32,
    pub projects: Vec<Project>,
    pub sessions: Vec<Session>,
}

impl Default for State {
    fn default() -> Self {
        Self {
            schema_version: SCHEMA_VERSION,
            projects: Vec::new(),
            sessions: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(default)]
pub struct Config {
    /// Comando do Claude Code (pode ser um caminho absoluto).
    pub claude_command: String,
    /// Argumentos extras passados a toda sessão.
    pub extra_args: Vec<String>,
    pub branch_prefix: String,
    pub default_worktree: bool,
    pub notifications: bool,
    pub bell: bool,
    /// Teclas (sintaxe do `tmux send-keys`) enviadas para aprovar/recusar (ADR-0007).
    pub approve_keys: Vec<String>,
    pub deny_keys: Vec<String>,
    /// Comando do diff completo (`D`); `{base}` vira a base da sessão (ADR-0009).
    pub diff_command: String,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            claude_command: "claude".into(),
            extra_args: Vec::new(),
            branch_prefix: "ccdeck/".into(),
            default_worktree: true,
            notifications: true,
            bell: true,
            approve_keys: vec!["1".into()],
            deny_keys: vec!["Escape".into()],
            diff_command: "git diff {base}".into(),
        }
    }
}

pub const PERMISSION_MODES: [&str; 4] = ["default", "acceptEdits", "plan", "auto"];

/// Rótulo na UI. `default` não passa `--permission-mode`, então vale o
/// `defaultMode` das settings do usuário (validacao-v0.2.md §6).
pub fn permission_mode_label(mode: &str) -> &str {
    if mode == PERMISSION_MODES[0] {
        "padrão das suas settings"
    } else {
        mode
    }
}

/// Último evento recebido de um hook, gravado em `status/<sid>.json`.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct StatusRecord {
    pub status: Status,
    pub event: String,
    #[serde(default)]
    pub detail: Option<String>,
    #[serde(default)]
    pub claude_session_id: Option<String>,
    pub at: DateTime<Utc>,
    /// Pedido de permissão aguardando resposta (só em `Approval`).
    #[serde(default)]
    pub pending: Option<ToolCall>,
    /// Última chamada de ferramenta vista (carregada adiante entre eventos).
    #[serde(default)]
    pub last_tool: Option<ToolCall>,
    /// Último texto do assistente (`Stop.last_assistant_message`).
    #[serde(default)]
    pub last_message: Option<String>,
    #[serde(default)]
    pub transcript_path: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ToolCall {
    pub tool: String,
    pub summary: String,
    /// `tool_use_id` do Claude Code; identifica o pedido nas travas do `y`/`x`.
    #[serde(default)]
    pub id: String,
}

/// Gera um id curto (8 hex) para sessões e projetos.
pub fn short_id() -> String {
    Uuid::new_v4().simple().to_string()[..8].to_string()
}

/// Converte um nome livre em slug seguro para branch e diretório.
pub fn slugify(name: &str) -> String {
    let mut out = String::new();
    let mut dash = false;
    for c in name.trim().chars() {
        let c = deunicode_char(c);
        if c.is_ascii_alphanumeric() {
            out.push(c.to_ascii_lowercase());
            dash = false;
        } else if !dash && !out.is_empty() {
            out.push('-');
            dash = true;
        }
    }
    while out.ends_with('-') {
        out.pop();
    }
    if out.is_empty() { "sessao".into() } else { out }
}

fn deunicode_char(c: char) -> char {
    match c {
        'á' | 'à' | 'â' | 'ã' | 'ä' | 'Á' | 'À' | 'Â' | 'Ã' => 'a',
        'é' | 'ê' | 'è' | 'É' | 'Ê' => 'e',
        'í' | 'Í' => 'i',
        'ó' | 'ô' | 'õ' | 'Ó' | 'Ô' | 'Õ' => 'o',
        'ú' | 'ü' | 'Ú' => 'u',
        'ç' | 'Ç' => 'c',
        _ => c,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn slug_basico() {
        assert_eq!(slugify("Migração Flyway V42"), "migracao-flyway-v42");
        assert_eq!(slugify("  fix/login  SSO!! "), "fix-login-sso");
        assert_eq!(slugify("***"), "sessao");
    }

    #[test]
    fn atencao() {
        assert!(Status::Approval.needs_attention());
        assert!(!Status::Working.needs_attention());
        assert!(!Status::Suspended.needs_attention());
    }
}
