//! Backend de sessões: servidor tmux dedicado `-L ccdeck` (ADR-0002).

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::process::{Command, ExitStatus, Stdio};

use anyhow::{Context, Result, bail};

pub const SOCKET: &str = "ccdeck";
pub const CLAUDE_WINDOW: &str = "claude";
pub const SETUP_WINDOW: &str = "setup";
pub const VERIFY_WINDOW: &str = "verify";

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
bind -n C-q detach-client
"##;

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
        c.args(["-u", "-L", SOCKET, "-f"]).arg(&self.conf);
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

    /// Sessões com o Claude Code vivo (janela `claude` existe).
    /// Servidor parado = conjunto vazio.
    pub fn list(&self) -> HashSet<String> {
        self.run(&[
            "list-windows",
            "-a",
            "-F",
            "#{session_name}\t#{window_name}",
        ])
        .map(|s| {
            s.lines()
                .filter_map(|l| l.split_once('\t'))
                .filter(|(_, w)| *w == CLAUDE_WINDOW)
                .map(|(s, _)| s.to_string())
                .collect()
        })
        .unwrap_or_default()
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

    fn claude_target(name: &str) -> String {
        format!("={name}:{CLAUDE_WINDOW}")
    }

    /// Teclas nomeadas (`Enter`, `Escape`, `1`…) para a janela do Claude.
    pub fn send_keys(&self, name: &str, keys: &[String]) -> Result<()> {
        let target = Self::claude_target(name);
        let mut args = vec!["send-keys", "-t", target.as_str()];
        args.extend(keys.iter().map(String::as_str));
        self.run(&args).map(|_| ())
    }

    /// Texto literal seguido de Enter.
    pub fn send_text(&self, name: &str, text: &str) -> Result<()> {
        let target = Self::claude_target(name);
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

    /// Tela visível do pane, com escapes ANSI (para o preview).
    pub fn capture(&self, name: &str) -> Option<Vec<u8>> {
        let out = self
            .cmd()
            .args(["capture-pane", "-p", "-e", "-t", &Self::claude_target(name)])
            .output()
            .ok()?;
        out.status.success().then_some(out.stdout)
    }

    /// Bloqueia até o usuário sair (Ctrl-q). Dentro de tmux usa um popup.
    pub fn attach(&self, name: &str) -> Result<ExitStatus> {
        let _ = self.run(&["select-window", "-t", &Self::claude_target(name)]);
        if std::env::var_os("TMUX").is_some() {
            let inner = format!(
                "env -u TMUX tmux -u -L {SOCKET} -f {} attach -t {}",
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
