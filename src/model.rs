//! Modelo de domínio: projetos, sessões, estados e configuração.

use std::collections::BTreeMap;
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
    /// USD por milhão de tokens, por prefixo de id de modelo (ADR-0010).
    pub prices: BTreeMap<String, Price>,
    /// `api` mostra o custo equivalente em API; `hidden` esconde (assinatura).
    pub cost_display: CostDisplay,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostDisplay {
    #[default]
    Api,
    Hidden,
}

/// Preço por milhão de tokens (USD).
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct Price {
    pub input: f64,
    pub output: f64,
    pub cache_write_5m: f64,
    pub cache_write_1h: f64,
    pub cache_read: f64,
}

/// Tabela de `platform.claude.com/docs/en/about-claude/pricing`, lida em
/// 2026-10-23. `+fast` = modo rápido (multiplicadores de cache por cima);
/// `+long` = prompt acima de 100 mil tokens, que só muda o preço do Haiku 5.5.
pub fn default_prices() -> BTreeMap<String, Price> {
    let p = |input, output, cache_write_5m, cache_write_1h, cache_read| Price {
        input,
        output,
        cache_write_5m,
        cache_write_1h,
        cache_read,
    };
    [
        ("claude-fable-5-1", p(10.0, 50.0, 12.5, 20.0, 0.25)),
        ("claude-mythos-5-1", p(10.0, 50.0, 12.5, 20.0, 0.25)),
        ("claude-fable-5", p(10.0, 50.0, 12.5, 20.0, 1.0)),
        ("claude-opus-5-5", p(4.0, 20.0, 5.0, 8.0, 0.20)),
        ("claude-opus-5-5+fast", p(8.0, 40.0, 10.0, 16.0, 0.40)),
        ("claude-opus-5", p(5.0, 25.0, 6.25, 10.0, 0.50)),
        ("claude-opus-5+fast", p(10.0, 50.0, 12.5, 20.0, 1.0)),
        ("claude-opus-4-8", p(5.0, 25.0, 6.25, 10.0, 0.50)),
        ("claude-opus-4-8+fast", p(10.0, 50.0, 12.5, 20.0, 1.0)),
        ("claude-sonnet-5-5", p(2.0, 10.0, 2.5, 4.0, 0.10)),
        ("claude-sonnet-5", p(2.0, 10.0, 2.5, 4.0, 0.20)),
        ("claude-haiku-5-5", p(0.10, 0.50, 0.125, 0.20, 0.01)),
        ("claude-haiku-5-5+long", p(0.50, 2.50, 0.625, 1.0, 0.05)),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_string(), v))
    .collect()
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
            prices: default_prices(),
            cost_display: CostDisplay::Api,
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
