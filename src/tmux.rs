//! Backend de sessões: servidor tmux dedicado `-L ccdeck` (ADR-0002).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use anyhow::{Context, Result, bail};

pub const SOCKET: &str = "ccdeck";

/// Servidor tmux do deck. `CCDECK_TMUX_SOCKET` troca o nome (testes sem
/// mexer nas sessões reais do usuário).
pub fn socket() -> String {
    std::env::var("CCDECK_TMUX_SOCKET")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| SOCKET.to_string())
}
pub const CLAUDE_WINDOW: &str = "claude";
pub const SETUP_WINDOW: &str = "setup";
pub const VERIFY_WINDOW: &str = "verify";
/// Sessão do modo integrado (ADR-0018): barra lateral + painel da sessão.
pub const UI_SESSION: &str = "ccdeck-ui";
/// Largura da barra lateral no modo integrado.
pub const SIDEBAR_COLS: &str = "46";
const PLACEHOLDER_CMD: &str = "sh -c 'clear; printf \"\\n  ◂ selecione uma sessão na barra (clique ou j/k)\\n\"; exec tail -f /dev/null'";

/// Painel do servidor do deck, com as marcas do ccdeck (ADR-0018).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PaneInfo {
    pub id: String,
    pub session: String,
    pub window: String,
    /// `@ccdeck_session`: nome da sessão do ccdeck dona deste Claude Code.
    pub tag: String,
    pub placeholder: bool,
}

impl PaneInfo {
    /// Sessão do ccdeck cujo Claude Code roda neste painel. Painéis sem
    /// marca (criados antes do ADR-0018) valem pela janela `claude`.
    pub fn owner(&self) -> Option<&str> {
        if !self.tag.is_empty() {
            return Some(&self.tag);
        }
        (self.window == CLAUDE_WINDOW && !self.placeholder && self.session != UI_SESSION)
            .then_some(self.session.as_str())
    }
}

pub fn parse_panes(out: &str) -> Vec<PaneInfo> {
    out.lines()
        .filter_map(|l| {
            let mut p = l.split('\t');
            Some(PaneInfo {
                id: p.next()?.to_string(),
                session: p.next()?.to_string(),
                window: p.next()?.to_string(),
                tag: p.next().unwrap_or("").to_string(),
                placeholder: p.next().unwrap_or("") == "1",
            })
        })
        .collect()
}

const CONF: &str = r##"# Gerado pelo ccdeck — não edite; é recriado a cada abertura.
set -g mouse on
set -g history-limit 50000
set -sg escape-time 10
# O Claude Code usa focus events (avisa na tela quando estão desligados).
set -g focus-events on
set -g default-terminal "tmux-256color"
set -g status on
set -g status-interval 2
set -g status-style "bg=#1a2029,fg=#d6dbe3"
set -g status-left-length 80
set -g status-right-length 60
set -g status-left "#[bg=#5fd7af,fg=#0b0e13,bold] ccdeck #[default] #{@ccdeck_label} "
set -g status-right "#[bg=#2a313c] Ctrl-q volta ao ccdeck "
set -g window-status-format " #W "
set -g window-status-current-format "#[bold] #W "
set -g base-index 0
set -g remain-on-exit off
# Modo integrado (ADR-0018): na sessão da direita, Ctrl-q volta para a barra.
bind -n C-q if-shell -F "#{&&:#{==:#{session_name},ccdeck-ui},#{!=:#{pane_index},0}}" "select-pane -t :.0" "detach-client"
"##;

/// Título da borda: a barra não tem; na sessão, diz se o teclado está nela.
const DECK_BORDER: &str = "#{?#{==:#{pane_index},0},,#{?pane_active,#[fg=#0b0e13#,bg=#5fd7af#,bold] ⌨ digitando aqui · Ctrl-q volta à árvore #[default],#[fg=#9aa4b2] clique aqui ou Enter para digitar #[default]}}";

#[derive(Debug, Clone)]
pub struct Tmux {
    conf: PathBuf,
}

impl Tmux {
    pub fn new(conf: PathBuf) -> Result<Self> {
        if let Some(dir) = conf.parent() {
            std::fs::create_dir_all(dir)?;
        }
        std::fs::write(&conf, CONF)?;
        let t = Self { conf };
        // Se o servidor já está de pé, recarrega a config.
        let _ = t.cmd().args(["source-file"]).arg(&t.conf).output();
        Ok(t)
    }

    fn cmd(&self) -> Command {
        let mut c = Command::new("tmux");
        // -u: força UTF-8 mesmo com locale mal configurado (nomes com acento).
        c.args(["-u", "-L", &socket(), "-f"]).arg(&self.conf);
        c.env_remove("TMUX");
        c
    }

    fn run(&self, args: &[&str]) -> Result<String> {
        let out = self
            .cmd()
            .args(args)
            .output()
            .context("falha ao executar tmux (está instalado?)")?;
        if !out.status.success() {
            bail!(
                "tmux {}: {}",
                args.first().unwrap_or(&""),
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(String::from_utf8_lossy(&out.stdout).to_string())
    }

    /// Todos os painéis do servidor do deck. Servidor parado = vazio.
    pub fn panes(&self) -> Vec<PaneInfo> {
        self.run(&[
            "list-panes",
            "-a",
            "-F",
            "#{pane_id}\t#{session_name}\t#{window_name}\t#{@ccdeck_session}\t#{@ccdeck_placeholder}",
        ])
        .map(|s| parse_panes(&s))
        .unwrap_or_default()
    }

    /// Sessões com o Claude Code vivo, onde quer que o painel esteja.
    pub fn list(&self) -> HashSet<String> {
        self.panes()
            .iter()
            .filter_map(|p| p.owner().map(str::to_string))
            .collect()
    }

    /// Painel do Claude Code da sessão (pela marca; ADR-0018).
    pub fn pane_of(&self, name: &str) -> Option<String> {
        self.panes()
            .into_iter()
            .find(|p| p.owner() == Some(name))
            .map(|p| p.id)
    }

    /// Marca painéis `claude` antigos, criados antes do ADR-0018.
    pub fn tag_legacy(&self) {
        for p in self.panes() {
            if p.tag.is_empty()
                && let Some(owner) = p.owner()
            {
                let owner = owner.to_string();
                let _ = self.run(&["set-option", "-p", "-t", &p.id, "@ccdeck_session", &owner]);
            }
        }
    }

    // --- modo integrado (ADR-0018) ------------------------------------------

    pub fn ui_exists(&self) -> bool {
        self.has_session(UI_SESSION)
    }

    /// Cria `ccdeck-ui`: barra lateral rodando `sidebar_cmd` + painel de espera.
    pub fn ui_create(
        &self,
        sidebar_cmd: &str,
        env: &[(String, String)],
        cols: u16,
        rows: u16,
    ) -> Result<()> {
        let (x, y) = (cols.max(80).to_string(), rows.max(20).to_string());
        let mut args: Vec<String> = [
            "new-session",
            "-d",
            "-s",
            UI_SESSION,
            "-n",
            "deck",
            "-x",
            &x,
            "-y",
            &y,
        ]
        .iter()
        .map(|s| s.to_string())
        .collect();
        for (k, v) in env {
            args.push("-e".into());
            args.push(format!("{k}={v}"));
        }
        args.push(sidebar_cmd.into());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run(&refs)?;
        let sidebar = self.run(&[
            "display-message",
            "-p",
            "-t",
            "=ccdeck-ui:deck.0",
            "#{pane_id}",
        ])?;
        self.new_placeholder(sidebar.trim())?;
        self.run(&["select-pane", "-t", sidebar.trim()])?;
        Ok(())
    }

    /// Painel de espera à direita da barra.
    pub fn new_placeholder(&self, sidebar: &str) -> Result<()> {
        let id = self.run(&[
            "split-window",
            "-h",
            "-d",
            "-l",
            "75%",
            "-t",
            sidebar,
            "-P",
            "-F",
            "#{pane_id}",
            PLACEHOLDER_CMD,
        ])?;
        self.run(&[
            "set-option",
            "-p",
            "-t",
            id.trim(),
            "@ccdeck_placeholder",
            "1",
        ])?;
        self.resize_sidebar(sidebar);
        Ok(())
    }

    /// Borda da janela do modo integrado: o painel com o teclado fica aceso
    /// e o título diz como voltar.
    pub fn style_deck(&self) {
        let w = format!("={UI_SESSION}:deck");
        for (opt, val) in [
            ("pane-border-status", "top"),
            // Com dois painéis o tmux pinta metade da divisória de cada cor;
            // sem indicadores e com cinza visível ela aparece inteira.
            ("pane-border-indicators", "off"),
            ("pane-border-style", "fg=#4b5563"),
            ("pane-active-border-style", "fg=#5fd7af"),
            ("pane-border-format", DECK_BORDER),
        ] {
            let _ = self.run(&["set-option", "-w", "-t", &w, opt, val]);
        }
    }

    pub fn resize_sidebar(&self, sidebar: &str) {
        let _ = self.run(&["resize-pane", "-t", sidebar, "-x", SIDEBAR_COLS]);
    }

    pub fn placeholder(&self) -> Option<String> {
        self.panes()
            .into_iter()
            .find(|p| p.placeholder)
            .map(|p| p.id)
    }

    /// Painéis da janela do modo integrado.
    pub fn deck_panes(&self) -> Vec<String> {
        self.run(&["list-panes", "-t", "=ccdeck-ui:deck", "-F", "#{pane_id}"])
            .map(|s| s.lines().map(str::to_string).collect())
            .unwrap_or_default()
    }

    /// Troca dois painéis de lugar sem mudar o foco.
    pub fn swap(&self, a: &str, b: &str) -> Result<()> {
        self.run(&["swap-pane", "-d", "-s", a, "-t", b]).map(|_| ())
    }

    /// Leva `pane` para a direita da barra (recuperação, ADR-0018).
    pub fn join_right(&self, pane: &str, sidebar: &str) -> Result<()> {
        self.run(&[
            "join-pane",
            "-h",
            "-d",
            "-l",
            "75%",
            "-s",
            pane,
            "-t",
            sidebar,
        ])?;
        self.resize_sidebar(sidebar);
        Ok(())
    }

    pub fn select_pane(&self, pane: &str) {
        let _ = self.run(&["select-pane", "-t", pane]);
    }

    pub fn zoomed(&self, pane: &str) -> bool {
        self.run(&["display-message", "-p", "-t", pane, "#{window_zoomed_flag}"])
            .is_ok_and(|s| s.trim() == "1")
    }

    pub fn toggle_zoom(&self, pane: &str) {
        let _ = self.run(&["resize-pane", "-Z", "-t", pane]);
    }

    pub fn ui_option(&self, key: &str) -> Option<String> {
        self.run(&["show-options", "-v", "-t", UI_SESSION, key])
            .ok()
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
    }

    pub fn set_ui_option(&self, key: &str, value: Option<&str>) {
        let _ = match value {
            Some(v) => self.run(&["set-option", "-t", UI_SESSION, key, v]),
            None => self.run(&["set-option", "-u", "-t", UI_SESSION, key]),
        };
    }

    /// Recria só a barra (painel 0) com outro comando; a sessão da direita
    /// fica como está. Usado quando o binário do ccdeck mudou.
    pub fn respawn_sidebar(&self, sidebar_cmd: &str, env: &[(String, String)]) -> Result<()> {
        let target = format!("={UI_SESSION}:deck.0");
        let mut args: Vec<String> = ["respawn-pane", "-k", "-t", &target]
            .iter()
            .map(|s| s.to_string())
            .collect();
        for (k, v) in env {
            args.push("-e".into());
            args.push(format!("{k}={v}"));
        }
        args.push(sidebar_cmd.into());
        let refs: Vec<&str> = args.iter().map(String::as_str).collect();
        self.run(&refs)?;
        Ok(())
    }

    pub fn kill_ui(&self) {
        let _ = self.run(&["kill-session", "-t", "=ccdeck-ui"]);
    }

    /// Entra no modo integrado (bloqueia até desconectar).
    pub fn attach_ui(&self) -> Result<ExitStatus> {
        if std::env::var_os("TMUX").is_some() {
            let inner = format!(
                "env -u TMUX tmux -u -L {} -f {} attach -t {UI_SESSION}",
                socket(),
                crate::hook::shell_quote(&self.conf.to_string_lossy()),
            );
            return Command::new("tmux")
                .args(["display-popup", "-E", "-w", "100%", "-h", "100%", &inner])
                .status()
                .context("tmux display-popup");
        }
        self.cmd()
            .args(["attach", "-t", UI_SESSION])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("tmux attach")
    }

    /// Nomes das janelas da sessão.
    pub fn windows(&self, name: &str) -> Vec<String> {
        self.run(&[
            "list-windows",
            "-t",
            &format!("={name}"),
            "-F",
            "#{window_name}",
        ])
        .map(|s| s.lines().map(str::to_string).collect())
        .unwrap_or_default()
    }

    pub fn kill_window(&self, name: &str, window: &str) -> Result<()> {
        self.run(&["kill-window", "-t", &format!("={name}:{window}")])
            .map(|_| ())
    }

    pub fn has_window(&self, name: &str, window: &str) -> bool {
        self.run(&[
            "list-windows",
            "-t",
            &format!("={name}"),
            "-F",
            "#{window_name}",
        ])
        .map(|s| s.lines().any(|w| w == window))
        .unwrap_or(false)
    }

    fn has_session(&self, name: &str) -> bool {
        self.run(&["has-session", "-t", &format!("={name}")])
            .is_ok()
    }

    /// Cria a sessão com a janela `claude`. Se a sessão já existe (por
    /// exemplo, só com a janela de setup), cria apenas a janela.
    pub fn new_session(
        &self,
        name: &str,
        cwd: &Path,
        env: &[(String, String)],
        label: &str,
        command: &str,
    ) -> Result<()> {
        let mut c = self.cmd();
        if self.has_session(name) {
            c.args([
                "new-window",
                "-t",
                &format!("={name}:"),
                "-n",
                CLAUDE_WINDOW,
                "-c",
            ])
            .arg(cwd);
        } else {
            c.args([
                "new-session",
                "-d",
                "-s",
                name,
                "-n",
                CLAUDE_WINDOW,
                "-x",
                "220",
                "-y",
                "50",
                "-c",
            ])
            .arg(cwd);
        }
        for (k, v) in env {
            c.arg("-e").arg(format!("{k}={v}"));
        }
        c.arg(command);
        let out = c.output().context("falha ao executar tmux")?;
        if !out.status.success() {
            bail!(
                "tmux new-session: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        // `=nome` não vale como alvo de set-option; o nome cd-<id> já é único.
        let _ = self.run(&["set-option", "-t", name, "@ccdeck_label", label]);
        // Marca o painel: ele pode mudar de lugar no modo integrado (ADR-0018).
        let _ = self.run(&[
            "set-option",
            "-p",
            "-t",
            &format!("={name}:{CLAUDE_WINDOW}"),
            "@ccdeck_session",
            name,
        ]);
        Ok(())
    }

    /// Janela auxiliar em segundo plano (ex.: setup do worktree).
    pub fn new_window(&self, name: &str, window: &str, cwd: &Path, command: &str) -> Result<()> {
        let mut c = self.cmd();
        c.args([
            "new-window",
            "-d",
            "-t",
            &format!("={name}:"),
            "-n",
            window,
            "-c",
        ])
        .arg(cwd)
        .arg(command);
        let out = c.output().context("falha ao executar tmux")?;
        if !out.status.success() {
            bail!(
                "tmux new-window: {}",
                String::from_utf8_lossy(&out.stderr).trim()
            );
        }
        Ok(())
    }

    /// Alvo para teclas e captura: o painel marcado, onde quer que esteja.
    fn target(&self, name: &str) -> String {
        self.pane_of(name)
            .unwrap_or_else(|| Self::claude_target(name))
    }

    fn claude_target(name: &str) -> String {
        format!("={name}:{CLAUDE_WINDOW}")
    }

    /// Teclas nomeadas (`Enter`, `Escape`, `1`…) para a janela do Claude.
    pub fn send_keys(&self, name: &str, keys: &[String]) -> Result<()> {
        let target = self.target(name);
        let mut args = vec!["send-keys", "-t", target.as_str()];
        args.extend(keys.iter().map(String::as_str));
        self.run(&args).map(|_| ())
    }

    /// Texto literal seguido de Enter.
    pub fn send_text(&self, name: &str, text: &str) -> Result<()> {
        let target = self.target(name);
        self.run(&["send-keys", "-t", &target, "-l", text])?;
        self.run(&["send-keys", "-t", &target, "Enter"]).map(|_| ())
    }

    pub fn set_label(&self, name: &str, label: &str) {
        let _ = self.run(&["set-option", "-t", name, "@ccdeck_label", label]);
    }

    pub fn kill(&self, name: &str) -> Result<()> {
        self.run(&["kill-session", "-t", &format!("={name}")])
            .map(|_| ())
    }

    /// Bloqueia até o usuário sair (Ctrl-q). Dentro de tmux usa um popup.
    /// Entra na sessão, na janela `window` (padrão: a do Claude Code).
    pub fn attach(&self, name: &str, window: Option<&str>) -> Result<ExitStatus> {
        let target = match window {
            Some(w) => format!("={name}:{w}"),
            None => Self::claude_target(name),
        };
        let _ = self.run(&["select-window", "-t", &target]);
        if std::env::var_os("TMUX").is_some() {
            let inner = format!(
                "env -u TMUX tmux -u -L {} -f {} attach -t {}",
                socket(),
                crate::hook::shell_quote(&self.conf.to_string_lossy()),
                crate::hook::shell_quote(name)
            );
            return Command::new("tmux")
                .args(["display-popup", "-E", "-w", "95%", "-h", "95%", &inner])
                .status()
                .context("tmux display-popup");
        }
        self.cmd()
            .args(["attach", "-t", name])
            .stdin(Stdio::inherit())
            .stdout(Stdio::inherit())
            .stderr(Stdio::inherit())
            .status()
            .context("tmux attach")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dono_do_painel() {
        let out = "%0\tccdeck-ui\tdeck\tcd-a\t\n\
                   %1\tcd-a\tclaude\t\t1\n\
                   %2\tcd-b\tclaude\t\t\n\
                   %3\tcd-b\tsetup\t\t\n\
                   %4\tccdeck-ui\tdeck\t\t\n";
        let p = parse_panes(out);
        let owners: Vec<Option<&str>> = p.iter().map(PaneInfo::owner).collect();
        // %0: claude de cd-a emprestado à direita; %1: espera na casa de cd-a;
        // %2: legado sem marca; %3: janela de setup; %4: a barra lateral.
        assert_eq!(owners, [Some("cd-a"), None, Some("cd-b"), None, None]);
        assert!(p[1].placeholder);
    }
}
