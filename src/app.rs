//! Estado da aplicação e ações (sem desenho; ver `ui.rs`).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::time::{Duration, Instant, SystemTime};

use anyhow::{Context, Result, bail};
use chrono::{DateTime, Utc};
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::text::Text;
use uuid::Uuid;

use crate::changes::{self, Job, Scanner};
use crate::git::{self, FileChange};
use crate::hook::{self, shell_quote};
use crate::input::{FormOutcome, SessionForm, TextInput};
use crate::model::{
    Config, PERMISSION_MODES, Project, Session, State, Status, StatusRecord, short_id, slugify,
};
use crate::notify;
use crate::paths::Paths;
use crate::project::{self, ProjectConfig};
use crate::store;
use crate::tmux::{SETUP_WINDOW, Tmux};
use crate::usage::{self, UsageScanner};

const LIST_EVERY: Duration = Duration::from_secs(2);
/// Tempo mínimo que um pedido precisa ficar na tela antes de `y`/`x` (ADR-0007).
const MIN_SEEN: Duration = Duration::from_millis(600);
/// Em *Trabalhando* sem evento novo por esse tempo, o painel avisa (ADR-0011).
const STALE_AFTER_MIN: i64 = 5;

/// Minutos sem evento de uma sessão em *Trabalhando*, quando passou do limite.
/// Interromper com `Esc` dentro da sessão não dispara hook (ADR-0011).
pub fn stale_minutes(status: Status, last_event: DateTime<Utc>, now: DateTime<Utc>) -> Option<i64> {
    let min = (now - last_event).num_minutes();
    (status == Status::Working && min >= STALE_AFTER_MIN).then_some(min)
}

#[derive(Debug, Clone)]
pub struct Runtime {
    pub status: Status,
    pub since: DateTime<Utc>,
    pub event: String,
    pub detail: Option<String>,
    pub record: Option<StatusRecord>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SetupState {
    Running,
    Ok,
    Failed(i32),
    Interrupted,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum View {
    Tree,
    Attention,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Item {
    Project(usize),
    Session(usize),
}

pub enum Modal {
    None,
    NewSession(Box<SessionForm>),
    NewProject(TextInput),
    Delete { session: String, dirty: usize },
    Notes { session: String, text: TextInput },
    Rename { session: String, text: TextInput },
    Reply { session: String, text: TextInput },
    Filter,
    Help,
}

pub struct Flash {
    pub text: String,
    pub error: bool,
    at: Instant,
}

pub struct App {
    pub paths: Paths,
    pub config: Config,
    pub state: State,
    pub tmux: Tmux,
    pub runtime: HashMap<String, Runtime>,
    pub alive: HashSet<String>,
    pub view: View,
    pub modal: Modal,
    pub cursor: usize,
    pub filter: TextInput,
    pub flash: Option<Flash>,
    pub preview: Text<'static>,
    pub quit: bool,
    pub pending_attach: Option<String>,
    /// Comando a rodar com a TUI suspensa (diff completo): (diretório, comando).
    pub pending_shell: Option<(PathBuf, String)>,
    pub scanner: Scanner,
    /// Uso de tokens por sessão, indexado em segundo plano (ADR-0010).
    pub usage: UsageScanner,
    /// Sessão → arquivo → outras sessões que alteraram o mesmo arquivo.
    pub collisions: HashMap<String, BTreeMap<String, Vec<String>>>,
    /// id do pedido → quando apareceu na tela pela primeira vez.
    seen_pending: HashMap<String, Instant>,
    /// Última resposta lida do transcript: sessão → (mtime, texto).
    msg_cache: HashMap<String, (Option<SystemTime>, Option<String>)>,
    last_list: Option<Instant>,
    initialized: bool,
}

impl App {
    pub fn new(paths: Paths, exe: &Path) -> Result<Self> {
        let config = store::load_config(&paths)?;
        let state = store::load_state(&paths)?;
        crate::hook::write_settings(&paths, exe)?;
        let tmux = Tmux::new(paths.tmux_conf())?;
        let mut app = Self {
            paths,
            config,
            state,
            tmux,
            runtime: HashMap::new(),
            alive: HashSet::new(),
            view: View::Tree,
            modal: Modal::None,
            cursor: 0,
            filter: TextInput::default(),
            flash: None,
            preview: Text::default(),
            quit: false,
            pending_attach: None,
            pending_shell: None,
            scanner: Scanner::default(),
            usage: UsageScanner::default(),
            collisions: HashMap::new(),
            seen_pending: HashMap::new(),
            msg_cache: HashMap::new(),
            last_list: None,
            initialized: false,
        };
        let ids: Vec<String> = app.state.sessions.iter().map(|s| s.id.clone()).collect();
        app.usage.preload(&app.paths, &ids);
        app.refresh();
        app.initialized = true;
        Ok(app)
    }

    // ---------- leitura ----------

    pub fn project(&self, id: &str) -> Option<&Project> {
        self.state.projects.iter().find(|p| p.id == id)
    }

    pub fn status_of(&self, s: &Session) -> Status {
        self.runtime
            .get(&s.id)
            .map(|r| r.status)
            .unwrap_or(Status::Suspended)
    }

    pub fn attention_count(&self) -> usize {
        self.state
            .sessions
            .iter()
            .filter(|s| self.status_of(s).needs_attention())
            .count()
    }

    fn matches_filter(&self, s: &Session) -> bool {
        let f = self.filter.value.trim().to_lowercase();
        if f.is_empty() {
            return true;
        }
        let proj = self
            .project(&s.project_id)
            .map(|p| p.name.to_lowercase())
            .unwrap_or_default();
        s.name.to_lowercase().contains(&f)
            || proj.contains(&f)
            || s.branch
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains(&f)
    }

    /// Linhas navegáveis da sidebar, na ordem de exibição.
    pub fn items(&self) -> Vec<Item> {
        let mut out = Vec::new();
        match self.view {
            View::Tree => {
                let filtering = !self.filter.value.trim().is_empty();
                for (pi, p) in self.state.projects.iter().enumerate() {
                    let sessions: Vec<usize> = self
                        .state
                        .sessions
                        .iter()
                        .enumerate()
                        .filter(|(_, s)| s.project_id == p.id && self.matches_filter(s))
                        .map(|(i, _)| i)
                        .collect();
                    if filtering && sessions.is_empty() {
                        continue;
                    }
                    out.push(Item::Project(pi));
                    if !p.collapsed || filtering {
                        out.extend(sessions.into_iter().map(Item::Session));
                    }
                }
            }
            View::Attention => {
                let mut v: Vec<(usize, Status, DateTime<Utc>)> = self
                    .state
                    .sessions
                    .iter()
                    .enumerate()
                    .filter(|(_, s)| self.matches_filter(s))
                    .filter_map(|(i, s)| {
                        let r = self.runtime.get(&s.id)?;
                        r.status.needs_attention().then_some((i, r.status, r.since))
                    })
                    .collect();
                v.sort_by_key(|(_, st, since)| (st.urgency(), *since));
                out.extend(v.into_iter().map(|(i, _, _)| Item::Session(i)));
            }
        }
        out
    }

    pub fn selected(&self) -> Option<Item> {
        self.items().get(self.cursor).copied()
    }

    pub fn selected_session(&self) -> Option<&Session> {
        match self.selected()? {
            Item::Session(i) => self.state.sessions.get(i),
            Item::Project(_) => None,
        }
    }

    /// Projeto do item selecionado (projeto ou sessão).
    pub fn selected_project(&self) -> Option<&Project> {
        match self.selected()? {
            Item::Project(i) => self.state.projects.get(i),
            Item::Session(i) => self.project(&self.state.sessions[i].project_id),
        }
    }

    // ---------- ciclo ----------

    pub fn tick(&mut self) {
        self.refresh();
        self.update_preview();
        self.update_changes();
        self.update_usage();
        self.mark_seen_pending();
        self.update_last_message();
        if let Some(f) = &self.flash
            && f.at.elapsed() > Duration::from_secs(6)
        {
            self.flash = None;
        }
    }

    fn job(s: &Session) -> Job {
        Job {
            session: s.id.clone(),
            cwd: s.cwd.clone(),
            base: s.base.clone(),
        }
    }

    /// Agenda a indexação de uso de todas as sessões (ADR-0010).
    fn update_usage(&mut self) {
        self.usage.collect();
        for s in &self.state.sessions {
            if let Some(t) = self
                .runtime
                .get(&s.id)
                .and_then(|r| r.record.as_ref())
                .and_then(|r| r.transcript_path.as_ref())
            {
                self.usage
                    .transcripts
                    .insert(s.id.clone(), PathBuf::from(t));
            }
        }
        if self.usage.busy()
            || self
                .usage
                .last
                .is_some_and(|t| t.elapsed() < changes::ALL_EVERY)
        {
            return;
        }
        let jobs = self
            .state
            .sessions
            .iter()
            .filter_map(|s| {
                let t = self.usage.transcripts.get(&s.id)?;
                Some(usage::Job {
                    session: s.id.clone(),
                    transcript: t.clone(),
                })
            })
            .collect();
        self.usage.spawn(&self.paths, jobs);
        self.usage.last = Some(Instant::now());
    }

    /// Agenda varreduras de alterações e recalcula colisões (ADR-0009).
    fn update_changes(&mut self) {
        if self.scanner.collect() {
            self.collisions = changes::collisions(&self.state.sessions, &self.scanner.results);
        }
        if self.scanner.busy() {
            return;
        }
        let now = Instant::now();
        if self
            .scanner
            .last_all
            .is_none_or(|t| t.elapsed() >= changes::ALL_EVERY)
        {
            let jobs = self.state.sessions.iter().map(Self::job).collect();
            self.scanner.spawn(jobs);
            self.scanner.last_all = Some(now);
            self.scanner.last_selected = Some(now);
        } else if self
            .scanner
            .last_selected
            .is_none_or(|t| t.elapsed() >= changes::SELECTED_EVERY)
            && let Some(s) = self.selected_session()
        {
            let job = Self::job(s);
            self.scanner.spawn(vec![job]);
            self.scanner.last_selected = Some(now);
        }
    }

    pub fn changes_of(&self, s: &Session) -> Option<&Vec<FileChange>> {
        self.scanner.results.get(&s.id)
    }

    pub fn session_name(&self, id: &str) -> String {
        self.state
            .sessions
            .iter()
            .find(|s| s.id == id)
            .map(|s| s.name.clone())
            .unwrap_or_else(|| id.to_string())
    }

    pub fn collision_count(&self) -> usize {
        self.collisions.len()
    }

    fn pending_of(&self, s: &Session) -> Option<&crate::model::ToolCall> {
        let r = self.runtime.get(&s.id)?;
        (r.status == Status::Approval)
            .then_some(r.record.as_ref()?.pending.as_ref())
            .flatten()
    }

    /// O pedido da sessão selecionada está na tela: registra quando apareceu.
    fn mark_seen_pending(&mut self) {
        let id = self
            .selected_session()
            .and_then(|s| self.pending_of(s))
            .map(|p| p.id.clone());
        if let Some(id) = id {
            self.seen_pending.entry(id).or_insert_with(Instant::now);
        }
        if self.seen_pending.len() > 256 {
            self.seen_pending
                .retain(|_, t| t.elapsed() < Duration::from_secs(3600));
        }
    }

    /// Última resposta: do hook (`Stop`) ou, na falta, do transcript.
    pub fn last_message(&self, s: &Session) -> Option<String> {
        let rec = self.runtime.get(&s.id).and_then(|r| r.record.as_ref());
        rec.and_then(|r| r.last_message.clone())
            .or_else(|| self.msg_cache.get(&s.id).and_then(|(_, m)| m.clone()))
    }

    fn update_last_message(&mut self) {
        let Some(s) = self.selected_session() else {
            return;
        };
        let rec = self.runtime.get(&s.id).and_then(|r| r.record.as_ref());
        if rec.is_some_and(|r| r.last_message.is_some()) {
            return;
        }
        let Some(path) = rec.and_then(|r| r.transcript_path.clone()) else {
            return;
        };
        let id = s.id.clone();
        let mtime = std::fs::metadata(&path).and_then(|m| m.modified()).ok();
        if self.msg_cache.get(&id).is_some_and(|(t, _)| *t == mtime) {
            return;
        }
        let msg = crate::transcript::last_assistant_text(Path::new(&path));
        self.msg_cache.insert(id, (mtime, msg));
    }

    fn setup_files(&self, sid: &str) -> (PathBuf, PathBuf) {
        let dir = self.paths.state_dir.join("setup");
        (
            dir.join(format!("{sid}.log")),
            dir.join(format!("{sid}.exit")),
        )
    }

    /// Estado do setup do worktree (ADR-0008). `None` = sem setup.
    pub fn setup_state(&self, s: &Session) -> Option<SetupState> {
        let (log, exit) = self.setup_files(&s.id);
        if let Ok(code) = std::fs::read_to_string(&exit) {
            return Some(match code.trim().parse::<i32>() {
                Ok(0) => SetupState::Ok,
                Ok(c) => SetupState::Failed(c),
                Err(_) => SetupState::Failed(-1),
            });
        }
        if !log.exists() {
            return None;
        }
        let any_window = self.tmux.has_window(&s.tmux_name(), SETUP_WINDOW);
        Some(if any_window {
            SetupState::Running
        } else {
            SetupState::Interrupted
        })
    }

    /// Registra ações enviadas às sessões (auditoria do `y`/`x`/`m`).
    fn log_action(&self, s: &Session, action: &str, detail: &str) {
        use std::io::Write;
        let line = format!(
            "{}\t{}\t{}\t{}\t{}\n",
            Utc::now().to_rfc3339(),
            s.id,
            s.name,
            action,
            detail.replace(['\n', '\t'], " ")
        );
        let path = self.paths.state_dir.join("actions.log");
        if let Ok(mut f) = std::fs::OpenOptions::new()
            .create(true)
            .append(true)
            .open(path)
        {
            let _ = f.write_all(line.as_bytes());
        }
    }

    /// `y` / `x`: responde ao pedido de permissão sem entrar (ADR-0007).
    pub fn answer_permission(&mut self, approve: bool) {
        let Some(s) = self.selected_session().cloned() else {
            return;
        };
        if !self.alive.contains(&s.tmux_name()) {
            self.error("a sessão não está rodando");
            return;
        }
        let fresh = store::load_status(&self.paths, &s.id);
        let Some(rec) = fresh.filter(|r| r.status == Status::Approval) else {
            self.error("a sessão não está aguardando aprovação");
            return;
        };
        let Some(pending) = rec.pending else {
            self.error("pedido desconhecido — Enter para entrar e decidir lá");
            return;
        };
        let seen_long_enough = self
            .seen_pending
            .get(&pending.id)
            .is_some_and(|t| t.elapsed() >= MIN_SEEN);
        if !seen_long_enough {
            self.error("o pedido acabou de mudar — confira e tente de novo");
            return;
        }
        let keys = if approve {
            self.config.approve_keys.clone()
        } else {
            self.config.deny_keys.clone()
        };
        match self.tmux.send_keys(&s.tmux_name(), &keys) {
            Ok(()) => {
                let action = if approve { "aprovar" } else { "recusar" };
                self.log_action(
                    &s,
                    action,
                    &format!("{}: {}", pending.tool, pending.summary),
                );
                self.seen_pending.remove(&pending.id);
                // A recusa não dispara hook no Claude Code (ADR-0011).
                if !approve {
                    match hook::record_local(
                        &self.paths,
                        &s.id,
                        rec.at,
                        Status::Yours,
                        "ccdeck · recusado",
                    ) {
                        Ok(_) => self.refresh(),
                        Err(e) => self.error(format!("falha ao gravar o status: {e:#}")),
                    }
                }
                self.info(format!(
                    "{}: {} enviado a {} ({})",
                    if approve { "Aprovado" } else { "Recusado" },
                    keys.join(" "),
                    s.name,
                    pending.tool
                ));
            }
            Err(e) => self.error(format!("falha ao enviar teclas: {e:#}")),
        }
    }

    pub fn open_reply(&mut self) {
        let Some(s) = self.selected_session() else {
            return;
        };
        match self.status_of(s) {
            Status::Yours | Status::Error => {
                self.modal = Modal::Reply {
                    session: s.id.clone(),
                    text: TextInput::new(""),
                };
            }
            Status::Approval => self.error("há um pedido pendente: use y/x ou Enter"),
            _ => self.error("a sessão não está esperando resposta"),
        }
    }

    fn send_reply(&mut self, id: &str, text: &str) -> Result<()> {
        let s = self
            .state
            .sessions
            .iter()
            .find(|s| s.id == id)
            .cloned()
            .context("sessão não existe")?;
        if !self.status_of(&s).needs_attention() || self.status_of(&s) == Status::Approval {
            bail!("a sessão não está mais esperando resposta");
        }
        self.tmux.send_text(&s.tmux_name(), text)?;
        self.log_action(&s, "responder", text);
        self.info(format!("resposta enviada a {}", s.name));
        Ok(())
    }

    /// `D`: diff completo com a TUI suspensa (ADR-0009).
    pub fn open_diff(&mut self) {
        let Some(s) = self.selected_session() else {
            return;
        };
        if !s.cwd.is_dir() {
            self.error("o diretório da sessão não existe mais");
            return;
        }
        let base = git::diff_base(&s.cwd, s.base.as_deref());
        let mut cmd = self
            .config
            .diff_command
            .replace("{base}", &shell_quote(&base));
        if cmd.trim_start().starts_with("git ") {
            cmd.push_str("; printf '\\n[Enter volta ao ccdeck] '; read _");
        }
        self.pending_shell = Some((s.cwd.clone(), cmd));
    }

    pub fn refresh(&mut self) {
        if self.last_list.is_none_or(|t| t.elapsed() >= LIST_EVERY) {
            self.alive = self.tmux.list();
            self.last_list = Some(Instant::now());
        }
        let records = store::load_status_records(&self.paths);
        let mut changed_ids = false;
        let mut alerts = Vec::new();
        for s in &mut self.state.sessions {
            let rec = records.get(&s.id);
            let alive = self.alive.contains(&s.tmux_name());
            let prev = self.runtime.get(&s.id);
            let next = match (alive, rec) {
                (true, Some(r)) => Runtime {
                    status: r.status,
                    since: r.at,
                    event: r.event.clone(),
                    detail: r.detail.clone(),
                    record: Some(r.clone()),
                },
                (true, None) => Runtime {
                    status: Status::Starting,
                    since: prev.map(|p| p.since).unwrap_or_else(Utc::now),
                    event: "—".into(),
                    detail: None,
                    record: None,
                },
                (false, r) => Runtime {
                    status: Status::Suspended,
                    since: prev
                        .filter(|p| p.status == Status::Suspended)
                        .map(|p| p.since)
                        .or(r.map(|r| r.at))
                        .unwrap_or(s.created_at),
                    event: r.map(|r| r.event.clone()).unwrap_or_else(|| "—".into()),
                    detail: None,
                    record: r.cloned(),
                },
            };
            if let Some(cid) = rec.and_then(|r| r.claude_session_id.as_deref())
                && let Ok(u) = Uuid::parse_str(cid)
                && u != s.claude_session_id
            {
                s.claude_session_id = u;
                changed_ids = true;
            }
            let became = prev.map(|p| p.status) != Some(next.status);
            if became && next.status.needs_attention() {
                alerts.push((
                    s.name.clone(),
                    s.project_id.clone(),
                    next.status,
                    next.detail.clone(),
                ));
            }
            self.runtime.insert(s.id.clone(), next);
        }
        if changed_ids {
            self.save();
        }
        if self.initialized {
            for (name, pid, st, detail) in alerts {
                let proj = self
                    .project(&pid)
                    .map(|p| p.name.clone())
                    .unwrap_or_default();
                if self.config.notifications {
                    let body = detail.unwrap_or_else(|| format!("{proj}: {}", st.label()));
                    notify::desktop(&format!("ccdeck · {name}"), &body);
                }
                if self.config.bell {
                    notify::bell();
                }
            }
        }
        let n = self.items().len();
        if self.cursor >= n {
            self.cursor = n.saturating_sub(1);
        }
    }

    fn update_preview(&mut self) {
        use ansi_to_tui::IntoText;
        let Some(s) = self.selected_session() else {
            self.preview = Text::default();
            return;
        };
        let name = s.tmux_name();
        if !self.alive.contains(&name) {
            self.preview = Text::default();
            return;
        }
        self.preview = self
            .tmux
            .capture(&name)
            .and_then(|b| b.into_text().ok())
            .unwrap_or_default();
    }

    fn save(&mut self) {
        if let Err(e) = store::save_state(&self.paths, &self.state) {
            self.error(format!("não foi possível salvar o estado: {e:#}"));
        }
    }

    pub fn info(&mut self, text: impl Into<String>) {
        self.flash = Some(Flash {
            text: text.into(),
            error: false,
            at: Instant::now(),
        });
    }

    pub fn error(&mut self, text: impl Into<String>) {
        self.flash = Some(Flash {
            text: text.into(),
            error: true,
            at: Instant::now(),
        });
    }

    // ---------- ações ----------

    pub fn add_project(&mut self, raw: &str) -> Result<()> {
        let expanded = expand_tilde(raw.trim());
        let path = std::fs::canonicalize(&expanded)
            .with_context(|| format!("diretório não encontrado: {}", expanded.display()))?;
        if !path.is_dir() {
            bail!("{} não é um diretório", path.display());
        }
        let path = git::repo_root(&path).unwrap_or(path);
        if self.state.projects.iter().any(|p| p.path == path) {
            bail!("projeto já cadastrado: {}", path.display());
        }
        let name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| path.display().to_string());
        self.state.projects.push(Project {
            id: short_id(),
            name: name.clone(),
            path,
            collapsed: false,
        });
        self.save();
        self.info(format!("projeto {name} adicionado"));
        self.cursor = self
            .items()
            .iter()
            .position(|i| *i == Item::Project(self.state.projects.len() - 1))
            .unwrap_or(0);
        Ok(())
    }

    pub fn open_new_session(&mut self) {
        let Some(p) = self.selected_project().cloned() else {
            self.error("cadastre um projeto primeiro (N)");
            return;
        };
        let is_repo = git::repo_root(&p.path).is_some();
        let base = git::current_branch(&p.path).unwrap_or_else(|| "main".into());
        let prep = self.prep_preview(&p);
        self.modal = Modal::NewSession(Box::new(SessionForm {
            prep,
            project_id: p.id,
            project_name: p.name,
            is_repo,
            name: TextInput::new(""),
            base: TextInput::new(base),
            worktree: is_repo && self.config.default_worktree,
            mode: 0,
            prompt: TextInput::new(""),
            focus: 0,
        }));
    }

    fn used_slots(&self, project_id: &str) -> Vec<u32> {
        self.state
            .sessions
            .iter()
            .filter(|s| s.project_id == project_id)
            .filter_map(|s| s.port_slot)
            .collect()
    }

    /// Linhas do bloco "Preparo do worktree" no formulário (ADR-0008).
    fn prep_preview(&self, p: &Project) -> Vec<String> {
        let cfg = match ProjectConfig::load(&p.path) {
            Ok(c) => c,
            Err(e) => return vec![format!("⚠ {}: {e:#}", project::FILE)],
        };
        if cfg.is_empty() {
            return vec![format!(
                "sem {} — `ccdeck init-project` cria um exemplo",
                project::FILE
            )];
        }
        let mut out = Vec::new();
        if !cfg.copy.is_empty() {
            out.push(format!("copiar  {}", cfg.copy.join(", ")));
        }
        if let Some(setup) = cfg.setup.as_deref().filter(|s| !s.trim().is_empty()) {
            out.push(format!("setup   {setup} (janela setup, em paralelo)"));
        }
        if cfg.ports.is_some() {
            let slot = project::free_slot(&self.used_slots(&p.id));
            let ports = cfg
                .ports_for(slot)
                .iter()
                .map(|p| format!("{}={}", p.name, p.port))
                .collect::<Vec<_>>()
                .join(" · ");
            out.push(format!("portas  {ports} (slot {slot})"));
        }
        if !cfg.env.is_empty() {
            out.push(format!(
                "env     {}",
                cfg.env.keys().cloned().collect::<Vec<_>>().join(", ")
            ));
        }
        out
    }

    pub fn create_session(&mut self, form: &SessionForm) -> Result<()> {
        let name = form.name.value.trim();
        if name.is_empty() {
            bail!("dê um nome à sessão");
        }
        let project = self
            .project(&form.project_id)
            .cloned()
            .context("projeto não existe mais")?;
        let cfg = ProjectConfig::load(&project.path)?;
        let slug = slugify(name);
        let use_worktree = form.worktree && form.is_repo;
        let base = form.base.value.trim().to_string();
        let mut notes = Vec::new();
        let (cwd, branch) = if use_worktree {
            let branch = format!("{}{}", self.config.branch_prefix, slug);
            let path = git::worktree_path(&project.path, &slug);
            git::worktree_add(&project.path, &path, &branch, &base)?;
            let (copied, warnings) = project::copy_files(&project.path, &path, &cfg.copy);
            if !copied.is_empty() {
                notes.push(format!("copiados: {}", copied.join(", ")));
            }
            notes.extend(warnings);
            (path, Some(branch))
        } else {
            (project.path.clone(), git::current_branch(&project.path))
        };
        let port_slot = cfg
            .ports
            .is_some()
            .then(|| project::free_slot(&self.used_slots(&project.id)));
        let ports = port_slot.map(|s| cfg.ports_for(s)).unwrap_or_default();
        let session = Session {
            id: short_id(),
            project_id: project.id.clone(),
            name: name.to_string(),
            cwd,
            branch,
            worktree: form.worktree && form.is_repo,
            claude_session_id: Uuid::new_v4(),
            permission_mode: form.mode().to_string(),
            notes: String::new(),
            created_at: Utc::now(),
            started_once: false,
            base: use_worktree.then_some(base),
            port_slot,
            ports,
        };
        let prompt = form.prompt.value.trim().to_string();
        self.state.sessions.push(session);
        let idx = self.state.sessions.len() - 1;
        self.save();
        self.start(idx, Some(prompt).filter(|p| !p.is_empty()))?;
        if use_worktree && let Some(setup) = cfg.setup.as_deref().filter(|s| !s.trim().is_empty()) {
            match self.run_setup(idx, setup) {
                Ok(()) => notes.push("setup rodando na janela setup".into()),
                Err(e) => notes.push(format!("setup não iniciou: {e:#}")),
            }
        }
        if !self.state.sessions[idx].ports.is_empty() {
            notes.push(
                self.state.sessions[idx]
                    .ports
                    .iter()
                    .map(|p| format!("{}={}", p.name, p.port))
                    .collect::<Vec<_>>()
                    .join(" "),
            );
        }
        if !notes.is_empty() {
            self.info(notes.join(" · "));
        }
        self.scanner.last_all = None;
        self.last_list = None;
        self.refresh();
        if let Some(pos) = self.items().iter().position(|i| *i == Item::Session(idx)) {
            self.cursor = pos;
        }
        self.pending_attach = Some(self.state.sessions[idx].tmux_name());
        Ok(())
    }

    /// Monta a linha de comando do Claude Code para iniciar ou retomar.
    pub fn claude_command(&self, s: &Session, resume: bool, prompt: Option<&str>) -> String {
        let proj = self
            .project(&s.project_id)
            .map(|p| p.name.as_str())
            .unwrap_or("");
        let mut parts: Vec<String> = vec![self.config.claude_command.clone()];
        parts.extend(self.config.extra_args.iter().map(|a| shell_quote(a)));
        if resume {
            parts.push("--resume".into());
        } else {
            parts.push("--session-id".into());
        }
        parts.push(s.claude_session_id.to_string());
        parts.push("-n".into());
        parts.push(shell_quote(&format!("{proj}/{}", s.name)));
        parts.push("--settings".into());
        parts.push(shell_quote(&self.paths.hooks_file().to_string_lossy()));
        if s.permission_mode != PERMISSION_MODES[0] {
            parts.push("--permission-mode".into());
            parts.push(shell_quote(&s.permission_mode));
        }
        if let Some(p) = prompt {
            parts.push(shell_quote(p));
        }
        parts.join(" ")
    }

    fn start(&mut self, idx: usize, prompt: Option<String>) -> Result<()> {
        let s = self.state.sessions[idx].clone();
        if !s.cwd.is_dir() {
            bail!("o diretório da sessão não existe mais: {}", s.cwd.display());
        }
        let cmd = self.claude_command(&s, s.started_once, prompt.as_deref());
        let label = self.label(&s);
        let env = self.session_env(&s);
        store::remove_status(&self.paths, &s.id);
        self.runtime.remove(&s.id);
        self.tmux
            .new_session(&s.tmux_name(), &s.cwd, &env, &label, &cmd)?;
        self.state.sessions[idx].started_once = true;
        self.save();
        Ok(())
    }

    /// Ambiente da sessão tmux: id do ccdeck, `env` do projeto e portas (ADR-0008).
    fn session_env(&self, s: &Session) -> Vec<(String, String)> {
        let mut env = vec![("CCDECK_SID".to_string(), s.id.clone())];
        if let Some(p) = self.project(&s.project_id)
            && let Ok(cfg) = ProjectConfig::load(&p.path)
        {
            env.extend(cfg.env);
        }
        env.extend(s.ports.iter().map(|p| (p.name.clone(), p.port.to_string())));
        env
    }

    /// Roda o `setup` numa janela própria, gravando log e código de saída.
    fn run_setup(&mut self, idx: usize, setup: &str) -> Result<()> {
        let s = self.state.sessions[idx].clone();
        let (log, exit) = self.setup_files(&s.id);
        if let Some(dir) = log.parent() {
            std::fs::create_dir_all(dir)?;
        }
        let _ = std::fs::remove_file(&exit);
        std::fs::write(&log, format!("$ {setup}\n"))?;
        let script = format!(
            "({setup}) 2>&1 | tee -a {log}; echo ${{PIPESTATUS[0]}} > {exit}",
            log = shell_quote(&log.to_string_lossy()),
            exit = shell_quote(&exit.to_string_lossy()),
        );
        let cmd = format!("bash -c {}", shell_quote(&script));
        self.tmux
            .new_window(&s.tmux_name(), SETUP_WINDOW, &s.cwd, &cmd)
    }

    fn label(&self, s: &Session) -> String {
        let proj = self
            .project(&s.project_id)
            .map(|p| p.name.as_str())
            .unwrap_or("");
        format!("{proj} ▸ {}", s.name)
    }

    pub fn enter(&mut self) {
        match self.selected() {
            Some(Item::Project(i)) => {
                let p = &mut self.state.projects[i];
                p.collapsed = !p.collapsed;
                self.save();
            }
            Some(Item::Session(i)) => {
                let name = self.state.sessions[i].tmux_name();
                if !self.alive.contains(&name)
                    && let Err(e) = self.start(i, None)
                {
                    self.error(format!("não foi possível retomar: {e:#}"));
                    return;
                }
                self.pending_attach = Some(name);
            }
            None => self.info("nada selecionado — N cadastra um projeto"),
        }
    }

    pub fn after_attach(&mut self, result: Result<std::process::ExitStatus>) {
        self.last_list = None;
        if let Err(e) = result {
            self.error(format!("falha ao entrar na sessão: {e:#}"));
        }
        self.refresh();
    }

    pub fn open_delete(&mut self) {
        if let Some(s) = self.selected_session() {
            let dirty = if s.worktree {
                git::dirty_count(&s.cwd)
            } else {
                0
            };
            self.modal = Modal::Delete {
                session: s.id.clone(),
                dirty,
            };
        } else if let Some(Item::Project(i)) = self.selected() {
            let p = &self.state.projects[i];
            if self.state.sessions.iter().any(|s| s.project_id == p.id) {
                self.error("remova as sessões do projeto antes de removê-lo");
            } else {
                let name = p.name.clone();
                self.state.projects.remove(i);
                self.save();
                self.info(format!(
                    "projeto {name} removido da lista (nada foi apagado do disco)"
                ));
            }
        }
    }

    /// option: 1 = só encerrar, 2 = remover worktree, 3 = forçar remoção.
    pub fn delete_session(&mut self, id: &str, option: u8) -> Result<()> {
        let idx = self
            .state
            .sessions
            .iter()
            .position(|s| s.id == id)
            .context("sessão não existe")?;
        let s = self.state.sessions[idx].clone();
        if option >= 2 && s.worktree {
            let repo = self
                .project(&s.project_id)
                .map(|p| p.path.clone())
                .context("projeto não existe")?;
            if s.cwd.exists() {
                git::worktree_remove(&repo, &s.cwd, option == 3)?;
            }
        }
        if self.alive.contains(&s.tmux_name()) {
            let _ = self.tmux.kill(&s.tmux_name());
        }
        store::remove_status(&self.paths, &s.id);
        let (log, exit) = self.setup_files(&s.id);
        let _ = std::fs::remove_file(log);
        let _ = std::fs::remove_file(exit);
        self.runtime.remove(&s.id);
        self.scanner.results.remove(&s.id);
        self.usage.hours.remove(&s.id);
        self.usage.transcripts.remove(&s.id);
        usage::remove(&self.paths, &s.id);
        crate::events::remove(&self.paths, &s.id);
        self.state.sessions.remove(idx);
        self.collisions = changes::collisions(&self.state.sessions, &self.scanner.results);
        self.save();
        self.last_list = None;
        let extra = match (option, s.worktree) {
            (1, true) => " (worktree e branch mantidos)",
            (_, true) => " (worktree removido; branch mantido)",
            _ => "",
        };
        self.info(format!("sessão {} encerrada{extra}", s.name));
        Ok(())
    }

    fn with_session(&mut self, id: &str, f: impl FnOnce(&mut Session)) {
        if let Some(s) = self.state.sessions.iter_mut().find(|s| s.id == id) {
            f(s);
        }
        self.save();
    }

    // ---------- teclado ----------

    pub fn handle_key(&mut self, key: KeyEvent) {
        let modal = std::mem::replace(&mut self.modal, Modal::None);
        self.modal = match modal {
            Modal::None => {
                self.handle_normal(key);
                return;
            }
            Modal::Help => match key.code {
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q') | KeyCode::Enter => {
                    Modal::None
                }
                _ => Modal::Help,
            },
            Modal::Filter => match key.code {
                KeyCode::Esc => {
                    self.filter = TextInput::default();
                    Modal::None
                }
                KeyCode::Enter => Modal::None,
                _ => {
                    self.filter.handle(key);
                    self.cursor = 0;
                    Modal::Filter
                }
            },
            Modal::NewProject(mut t) => match key.code {
                KeyCode::Esc => Modal::None,
                KeyCode::Enter => match self.add_project(&t.value) {
                    Ok(()) => Modal::None,
                    Err(e) => {
                        self.error(format!("{e:#}"));
                        Modal::NewProject(t)
                    }
                },
                _ => {
                    t.handle(key);
                    Modal::NewProject(t)
                }
            },
            Modal::NewSession(mut form) => match form.handle(key) {
                FormOutcome::Continue => Modal::NewSession(form),
                FormOutcome::Cancel => Modal::None,
                FormOutcome::Submit => match self.create_session(&form) {
                    Ok(()) => Modal::None,
                    Err(e) => {
                        self.error(format!("{e:#}"));
                        Modal::NewSession(form)
                    }
                },
            },
            Modal::Delete { session, dirty } => {
                let opt = match key.code {
                    KeyCode::Char('1') => Some(1),
                    KeyCode::Char('2') => Some(2),
                    KeyCode::Char('3') => Some(3),
                    KeyCode::Esc | KeyCode::Char('q') => return,
                    _ => None,
                };
                match opt {
                    Some(o) => {
                        if let Err(e) = self.delete_session(&session, o) {
                            self.error(format!("{e:#}"));
                        }
                        Modal::None
                    }
                    None => Modal::Delete { session, dirty },
                }
            }
            Modal::Notes { session, mut text } => {
                if key.code == KeyCode::Esc {
                    Modal::None
                } else if key.code == KeyCode::Char('s')
                    && key.modifiers.contains(KeyModifiers::CONTROL)
                {
                    let v = text.value.clone();
                    self.with_session(&session, |s| s.notes = v);
                    self.info("notas salvas");
                    Modal::None
                } else {
                    text.handle(key);
                    Modal::Notes { session, text }
                }
            }
            Modal::Reply { session, mut text } => match key.code {
                KeyCode::Esc => Modal::None,
                KeyCode::Enter if !text.value.trim().is_empty() => {
                    let v = text.value.trim().to_string();
                    match self.send_reply(&session, &v) {
                        Ok(()) => Modal::None,
                        Err(e) => {
                            self.error(format!("{e:#}"));
                            Modal::Reply { session, text }
                        }
                    }
                }
                _ => {
                    text.handle(key);
                    Modal::Reply { session, text }
                }
            },
            Modal::Rename { session, mut text } => match key.code {
                KeyCode::Esc => Modal::None,
                KeyCode::Enter if !text.value.trim().is_empty() => {
                    let v = text.value.trim().to_string();
                    self.with_session(&session, |s| s.name = v);
                    if let Some(s) = self.state.sessions.iter().find(|s| s.id == session) {
                        self.tmux.set_label(&s.tmux_name(), &self.label(s));
                    }
                    Modal::None
                }
                _ => {
                    text.handle(key);
                    Modal::Rename { session, text }
                }
            },
        };
    }

    fn handle_normal(&mut self, key: KeyEvent) {
        let n = self.items().len();
        match key.code {
            // Ctrl-q é o "voltar" de dentro da sessão; aqui não pode sair do app.
            KeyCode::Char('q') if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.quit = true
            }
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => self.quit = true,
            KeyCode::Down | KeyCode::Char('j') if n > 0 => {
                self.cursor = (self.cursor + 1).min(n - 1)
            }
            KeyCode::Up | KeyCode::Char('k') => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Home | KeyCode::Char('g') => self.cursor = 0,
            KeyCode::End | KeyCode::Char('G') => self.cursor = n.saturating_sub(1),
            KeyCode::Left | KeyCode::Char('h') => self.set_collapsed(true),
            KeyCode::Right | KeyCode::Char('l') => self.set_collapsed(false),
            KeyCode::Enter => self.enter(),
            KeyCode::Char('n') => self.open_new_session(),
            KeyCode::Char('N') => {
                let cwd = std::env::current_dir()
                    .map(|p| p.display().to_string())
                    .unwrap_or_default();
                self.modal = Modal::NewProject(TextInput::new(cwd));
            }
            KeyCode::Char('a') | KeyCode::Tab => {
                self.view = if self.view == View::Tree {
                    View::Attention
                } else {
                    View::Tree
                };
                self.cursor = 0;
            }
            KeyCode::Char('e') => {
                if let Some(s) = self.selected_session() {
                    self.modal = Modal::Notes {
                        session: s.id.clone(),
                        text: TextInput::multiline(s.notes.clone()),
                    };
                }
            }
            KeyCode::Char('r') => {
                if let Some(s) = self.selected_session() {
                    self.modal = Modal::Rename {
                        session: s.id.clone(),
                        text: TextInput::new(s.name.clone()),
                    };
                }
            }
            KeyCode::Char('d') => self.open_delete(),
            KeyCode::Char('y') => self.answer_permission(true),
            KeyCode::Char('x') => self.answer_permission(false),
            KeyCode::Char('m') => self.open_reply(),
            KeyCode::Char('D') => self.open_diff(),
            KeyCode::Char('/') => self.modal = Modal::Filter,
            KeyCode::Char('?') => self.modal = Modal::Help,
            KeyCode::Esc => {
                self.filter = TextInput::default();
            }
            _ => {}
        }
    }

    fn set_collapsed(&mut self, collapsed: bool) {
        if self.view != View::Tree {
            return;
        }
        let pi = match self.selected() {
            Some(Item::Project(i)) => i,
            Some(Item::Session(i)) => {
                let pid = self.state.sessions[i].project_id.clone();
                match self.state.projects.iter().position(|p| p.id == pid) {
                    Some(p) => p,
                    None => return,
                }
            }
            None => return,
        };
        if self.state.projects[pi].collapsed != collapsed {
            self.state.projects[pi].collapsed = collapsed;
            self.save();
        }
        if collapsed && let Some(pos) = self.items().iter().position(|i| *i == Item::Project(pi)) {
            self.cursor = pos;
        }
    }
}

pub fn expand_tilde(p: &str) -> PathBuf {
    if let Some(rest) = p.strip_prefix("~/")
        && let Some(home) = std::env::var_os("HOME")
    {
        return PathBuf::from(home).join(rest);
    }
    PathBuf::from(p)
}

/// "há 3 min", "há 2 h"…
pub fn ago(t: DateTime<Utc>) -> String {
    let secs = (Utc::now() - t).num_seconds().max(0);
    match secs {
        0..=59 => format!("{secs} s"),
        60..=3599 => format!("{} min", secs / 60),
        3600..=86_399 => format!("{} h", secs / 3600),
        _ => format!("{} d", secs / 86_400),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn aviso_sem_eventos() {
        let now = Utc::now();
        let ha = |m| now - chrono::Duration::minutes(m);
        assert_eq!(stale_minutes(Status::Working, ha(4), now), None);
        assert_eq!(stale_minutes(Status::Working, ha(7), now), Some(7));
        assert_eq!(stale_minutes(Status::Approval, ha(30), now), None);
        assert_eq!(stale_minutes(Status::Yours, ha(30), now), None);
    }
}
