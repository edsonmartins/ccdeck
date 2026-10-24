//! ccdeck — gerenciador de sessões do Claude Code no terminal.

mod app;
mod changes;
mod events;
mod git;
mod hook;
mod input;
mod model;
mod notify;
mod openspec;
mod paths;
mod project;
mod store;
mod summary;
mod timeline;
mod tmux;
mod transcript;
mod ui;
mod usage;

use std::process::Command;
use std::time::Duration;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use ratatui::crossterm::event::{self, Event, KeyEventKind};

use crate::app::App;
use crate::paths::Paths;

#[derive(Parser)]
#[command(
    name = "ccdeck",
    version,
    about = "Organize várias sessões do Claude Code no terminal"
)]
struct Cli {
    #[command(subcommand)]
    cmd: Option<Cmd>,
}

#[derive(Subcommand)]
enum Cmd {
    /// Usado pelos hooks do Claude Code (lê o evento do stdin). Não chame à mão.
    Hook,
    /// Verifica dependências e mostra onde ficam os arquivos.
    Doctor,
    /// Cria um `.ccdeck.json` de exemplo no diretório (padrão: o atual).
    InitProject { dir: Option<std::path::PathBuf> },
}

fn main() -> Result<()> {
    let cli = Cli::parse();
    let paths = Paths::resolve()?;
    match cli.cmd {
        Some(Cmd::Hook) => {
            hook::run(&paths);
            Ok(())
        }
        Some(Cmd::Doctor) => doctor(&paths),
        Some(Cmd::InitProject { dir }) => {
            let dir = match dir {
                Some(d) => d,
                None => std::env::current_dir()?,
            };
            let path = project::init(&dir)?;
            println!(
                "criado {} — ajuste copy, setup, env e ports",
                path.display()
            );
            Ok(())
        }
        None => run_tui(paths),
    }
}

fn run_tui(paths: Paths) -> Result<()> {
    for bin in ["tmux", "git"] {
        if which(bin).is_none() {
            bail!("{bin} não encontrado no PATH — rode `ccdeck doctor`");
        }
    }
    let exe = std::env::current_exe()?;
    let mut app = App::new(paths, &exe)?;
    let mut terminal = ratatui::init();
    let result = (|| -> Result<()> {
        loop {
            app.tick();
            terminal.draw(|f| ui::draw(f, &app))?;
            if event::poll(Duration::from_millis(250))?
                && let Event::Key(key) = event::read()?
                && key.kind == KeyEventKind::Press
            {
                app.handle_key(key);
            }
            if let Some(name) = app.pending_attach.take() {
                ratatui::restore();
                let r = app.tmux.attach(&name);
                terminal = ratatui::init();
                terminal.clear()?;
                app.after_attach(r);
            }
            if let Some((dir, cmd)) = app.pending_shell.take() {
                ratatui::restore();
                let mut c = Command::new("sh");
                c.args(["-c", &cmd]).current_dir(&dir);
                if std::env::var_os("LESSCHARSET").is_none() {
                    c.env("LESSCHARSET", "utf-8");
                }
                let r = c.status();
                terminal = ratatui::init();
                terminal.clear()?;
                if let Err(e) = r {
                    app.error(format!("falha ao rodar {cmd}: {e}"));
                }
            }
            if app.quit {
                return Ok(());
            }
        }
    })();
    ratatui::restore();
    result
}

fn which(bin: &str) -> Option<String> {
    let out = Command::new("sh")
        .args(["-c", &format!("command -v {bin}")])
        .output()
        .ok()?;
    out.status
        .success()
        .then(|| String::from_utf8_lossy(&out.stdout).trim().to_string())
}

fn version(bin: &str, arg: &str) -> String {
    Command::new(bin)
        .arg(arg)
        .output()
        .ok()
        .map(|o| {
            String::from_utf8_lossy(&o.stdout)
                .lines()
                .next()
                .unwrap_or("")
                .trim()
                .to_string()
        })
        .unwrap_or_default()
}

fn doctor(paths: &Paths) -> Result<()> {
    let cfg = store::load_config(paths)?;
    let checks = [
        ("tmux", "-V", "≥ 3.2"),
        ("git", "--version", "≥ 2.20"),
        (cfg.claude_command.as_str(), "--version", "Claude Code"),
    ];
    let mut missing = Vec::new();
    for (bin, arg, req) in checks {
        match which(bin) {
            Some(p) => println!("ok   {bin:<8} {}  ({p})", version(bin, arg)),
            None => {
                println!("FALTA {bin:<8} {req}");
                missing.push(bin.to_string());
            }
        }
    }
    println!();
    println!("config  {}", paths.config_file.display());
    println!("estado  {}", paths.state_file.display());
    println!("hooks   {}", paths.hooks_file().display());
    println!("status  {}", paths.status_dir().display());
    println!("tmux    tmux -L {} (sessões do deck)", tmux::SOCKET);
    usage_report(paths, &cfg);
    if !missing.is_empty() {
        bail!("dependências faltando: {}", missing.join(", "));
    }
    Ok(())
}

/// Uso já indexado e modelos sem preço em `prices` (ADR-0010).
fn usage_report(paths: &Paths, cfg: &model::Config) {
    let Ok(state) = store::load_state(paths) else {
        return;
    };
    let mut by_model: std::collections::BTreeMap<String, usage::Tokens> = Default::default();
    for s in &state.sessions {
        for models in usage::load(paths, &s.id).hours.values() {
            for (m, t) in models {
                by_model.entry(m.clone()).or_default().add(t);
            }
        }
    }
    if by_model.is_empty() {
        return;
    }
    let mut total = 0.0;
    let mut unpriced = std::collections::BTreeSet::new();
    for (m, t) in &by_model {
        match usage::cost(&cfg.prices, m, t) {
            Some(c) => total += c,
            None => {
                unpriced.insert(usage::display_model(m).to_string());
            }
        }
    }
    println!();
    if cfg.cost_display == model::CostDisplay::Api {
        println!("uso     custo equivalente em API (sessões do ccdeck): US$ {total:.2}");
    }
    for m in unpriced {
        println!("AVISO  modelo sem preço em config.json (prices): {m}");
    }
}
