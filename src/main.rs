//! ccdeck — gerenciador de sessões do Claude Code no terminal.

mod app;
mod broadcast;
mod changes;
mod delivery;
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
mod windows;

use std::process::Command;
use std::time::Duration;

use anyhow::{Result, bail};
use clap::{Parser, Subcommand};
use ratatui::crossterm::event::{self, Event, KeyEventKind, MouseButton, MouseEventKind};

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
    /// Abre o ccdeck (o mesmo que sem subcomando; ADR-0018).
    Ui,
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
        Some(Cmd::Ui) | None => match std::env::var("CCDECK_UI_PANE") {
            // Já dentro de `ccdeck-ui`: esta é a barra lateral.
            Ok(_) => match std::env::var("TMUX_PANE") {
                Ok(pane) => run_tui(paths, pane),
                Err(_) => bail!("CCDECK_UI_PANE sem TMUX_PANE: rode só `ccdeck`"),
            },
            Err(_) => open_ui(&paths),
        },
    }
}

/// Cria (ou reaproveita) a sessão `ccdeck-ui` e entra nela (ADR-0018).
fn open_ui(paths: &Paths) -> Result<()> {
    for bin in ["tmux", "git"] {
        if which(bin).is_none() {
            bail!("{bin} não encontrado no PATH — rode `ccdeck doctor`");
        }
    }
    let t = tmux::Tmux::new(paths.tmux_conf())?;
    if !t.ui_exists() {
        let exe = std::env::current_exe()?;
        let cmd = format!("{} ui", hook::shell_quote(&exe.to_string_lossy()));
        let mut env = vec![("CCDECK_UI_PANE".to_string(), "1".to_string())];
        if let Some(home) = &paths.home {
            env.push(("CCDECK_HOME".into(), home.to_string_lossy().to_string()));
        }
        if let Ok(sock) = std::env::var("CCDECK_TMUX_SOCKET") {
            env.push(("CCDECK_TMUX_SOCKET".into(), sock));
        }
        let (cols, rows) = ratatui::crossterm::terminal::size().unwrap_or((200, 50));
        t.ui_create(&cmd, &env, cols, rows)?;
    }
    t.attach_ui()?;
    Ok(())
}

/// A TUI é a barra lateral de `ccdeck-ui`; `sidebar` é o painel dela (ADR-0018).
fn run_tui(paths: Paths, sidebar: String) -> Result<()> {
    for bin in ["tmux", "git"] {
        if which(bin).is_none() {
            bail!("{bin} não encontrado no PATH — rode `ccdeck doctor`");
        }
    }
    let exe = std::env::current_exe()?;
    let mut app = App::new(paths, &exe)?;
    app.enable_ui(sidebar);
    let mouse = |on: bool| {
        use ratatui::crossterm::event::{
            DisableFocusChange, DisableMouseCapture, EnableFocusChange, EnableMouseCapture,
        };
        let _ = if on {
            ratatui::crossterm::execute!(std::io::stdout(), EnableMouseCapture, EnableFocusChange)
        } else {
            ratatui::crossterm::execute!(std::io::stdout(), DisableMouseCapture, DisableFocusChange)
        };
    };
    let mut terminal = ratatui::init();
    mouse(true);
    let result = (|| -> Result<()> {
        loop {
            app.tick();
            terminal.draw(|f| ui::draw(f, &app))?;
            if event::poll(Duration::from_millis(250))? {
                match event::read()? {
                    Event::Key(key) if key.kind == KeyEventKind::Press => app.handle_key(key),
                    Event::FocusGained => app.focused = true,
                    Event::FocusLost => app.focused = false,
                    Event::Mouse(m) => match m.kind {
                        MouseEventKind::Down(MouseButton::Left) => app.click(m.column, m.row),
                        MouseEventKind::ScrollDown => app.scroll(true),
                        MouseEventKind::ScrollUp => app.scroll(false),
                        _ => {}
                    },
                    _ => {}
                }
            }
            if let Some(name) = app.pending_attach.take() {
                mouse(false);
                ratatui::restore();
                let window = app.pending_window.take();
                let r = app.tmux.attach(&name, window.as_deref());
                terminal = ratatui::init();
                terminal.clear()?;
                mouse(true);
                app.after_attach(r);
            }
            if let Some((dir, cmd)) = app.pending_shell.take() {
                mouse(false);
                app.set_shell_zoom(true);
                ratatui::restore();
                let mut c = Command::new("sh");
                c.args(["-c", &cmd]).current_dir(&dir);
                if std::env::var_os("LESSCHARSET").is_none() {
                    c.env("LESSCHARSET", "utf-8");
                }
                let r = c.status();
                terminal = ratatui::init();
                terminal.clear()?;
                mouse(true);
                app.set_shell_zoom(false);
                if let Err(e) = r {
                    app.error(format!("falha ao rodar {cmd}: {e}"));
                }
            }
            if app.quit {
                return Ok(());
            }
        }
    })();
    mouse(false);
    ratatui::restore();
    if app.quit {
        app.leave_ui();
    }
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
    println!("tmux    tmux -L {} (sessões do deck)", tmux::socket());
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
