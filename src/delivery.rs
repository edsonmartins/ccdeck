//! Entrega da sessão (commit, push, PR) e status do CI via `gh` (ADR-0017).

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::mpsc::{Receiver, Sender, channel};
use std::time::{Duration, Instant};

use anyhow::{Context, Result, bail};
use serde_json::Value;

pub const CI_EVERY: Duration = Duration::from_secs(60);

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Ci {
    NoChecks,
    Ok,
    Pending(usize),
    Failing(usize),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Pr {
    pub number: u64,
    pub url: String,
    pub head: String,
    pub title: String,
    pub ci: Ci,
}

const FAILED: [&str; 7] = [
    "FAILURE",
    "TIMED_OUT",
    "CANCELLED",
    "ACTION_REQUIRED",
    "STARTUP_FAILURE",
    "ERROR",
    "STALE",
];

/// Resume o `statusCheckRollup` (CheckRun e StatusContext). Itens que não
/// entendemos são ignorados.
pub fn ci_of(rollup: &[Value]) -> Ci {
    let (mut fail, mut pending, mut known) = (0, 0, 0);
    for c in rollup {
        let s = |k: &str| c.get(k).and_then(Value::as_str).map(str::to_uppercase);
        let (status, conclusion, state) = (s("status"), s("conclusion"), s("state"));
        if status.is_none() && conclusion.is_none() && state.is_none() {
            continue;
        }
        known += 1;
        let failed = conclusion.as_deref().is_some_and(|v| FAILED.contains(&v))
            || state
                .as_deref()
                .is_some_and(|v| v == "FAILURE" || v == "ERROR");
        let running = status.as_deref().is_some_and(|v| v != "COMPLETED")
            || state
                .as_deref()
                .is_some_and(|v| v == "PENDING" || v == "EXPECTED");
        if failed {
            fail += 1;
        } else if running {
            pending += 1;
        }
    }
    match (known, fail, pending) {
        (0, _, _) => Ci::NoChecks,
        (_, f, _) if f > 0 => Ci::Failing(f),
        (_, _, p) if p > 0 => Ci::Pending(p),
        _ => Ci::Ok,
    }
}

/// Saída de `gh pr list --json number,url,headRefName,title,statusCheckRollup`.
pub fn parse_prs(json: &str) -> Result<Vec<Pr>> {
    let v: Value = serde_json::from_str(json).context("saída do gh não é JSON")?;
    let arr = v.as_array().context("saída do gh não é uma lista")?;
    Ok(arr
        .iter()
        .filter_map(|p| {
            Some(Pr {
                number: p.get("number")?.as_u64()?,
                url: p.get("url").and_then(Value::as_str).unwrap_or("").into(),
                head: p.get("headRefName")?.as_str()?.into(),
                title: p.get("title").and_then(Value::as_str).unwrap_or("").into(),
                ci: ci_of(
                    p.get("statusCheckRollup")
                        .and_then(Value::as_array)
                        .map(Vec::as_slice)
                        .unwrap_or_default(),
                ),
            })
        })
        .collect())
}

fn run(dir: &Path, program: &str, args: &[&str]) -> Result<String> {
    let out = Command::new(program)
        .args(args)
        .current_dir(dir)
        .env("GH_PROMPT_DISABLED", "1")
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .with_context(|| format!("falha ao executar {program}"))?;
    let text = format!(
        "{}{}",
        String::from_utf8_lossy(&out.stdout),
        String::from_utf8_lossy(&out.stderr)
    )
    .trim()
    .to_string();
    if !out.status.success() {
        bail!("{program} {}: {text}", args.first().unwrap_or(&""));
    }
    Ok(text)
}

pub fn list_prs(gh: &str, dir: &Path) -> Result<Vec<Pr>> {
    let out = Command::new(gh)
        .args([
            "pr",
            "list",
            "--state",
            "open",
            "--limit",
            "100",
            "--json",
            "number,url,headRefName,title,statusCheckRollup",
        ])
        .current_dir(dir)
        .env("GH_PROMPT_DISABLED", "1")
        .output()
        .with_context(|| format!("falha ao executar {gh}"))?;
    if !out.status.success() {
        bail!("{}", String::from_utf8_lossy(&out.stderr).trim());
    }
    parse_prs(&String::from_utf8_lossy(&out.stdout))
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Action {
    Commit { message: String },
    Push,
    Pr { title: String, body: String },
}

impl Action {
    pub fn label(&self) -> &'static str {
        match self {
            Action::Commit { .. } => "commit",
            Action::Push => "push",
            Action::Pr { .. } => "PR",
        }
    }
}

/// Executa a ação no worktree. Devolve a saída do git/gh.
pub fn execute(gh: &str, cwd: &Path, branch: &str, base: &str, action: &Action) -> Result<String> {
    match action {
        Action::Commit { message } => {
            run(cwd, "git", &["add", "-A"])?;
            run(cwd, "git", &["commit", "-m", message])
        }
        Action::Push => run(cwd, "git", &["push", "-u", "origin", branch]),
        Action::Pr { title, body } => run(
            cwd,
            gh,
            &[
                "pr", "create", "--base", base, "--head", branch, "--title", title, "--body", body,
            ],
        ),
    }
}

// --- segundo plano ----------------------------------------------------------

type CiMsg = (String, Result<Vec<Pr>, String>);

/// Consulta os PRs abertos por projeto (ADR-0017).
pub struct CiScanner {
    tx: Sender<CiMsg>,
    rx: Receiver<CiMsg>,
    in_flight: usize,
    pub last: Option<Instant>,
    /// Projeto → PRs abertos.
    pub prs: HashMap<String, Vec<Pr>>,
    /// Projeto → último erro do `gh`.
    pub errors: HashMap<String, String>,
}

impl Default for CiScanner {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            tx,
            rx,
            in_flight: 0,
            last: None,
            prs: HashMap::new(),
            errors: HashMap::new(),
        }
    }
}

impl CiScanner {
    pub fn busy(&self) -> bool {
        self.in_flight > 0
    }

    /// (id do projeto, diretório)
    pub fn spawn(&mut self, gh: &str, projects: Vec<(String, PathBuf)>) {
        if projects.is_empty() {
            return;
        }
        self.in_flight += projects.len();
        let tx = self.tx.clone();
        let gh = gh.to_string();
        std::thread::spawn(move || {
            for (id, dir) in projects {
                let r = list_prs(&gh, &dir).map_err(|e| format!("{e:#}"));
                if tx.send((id, r)).is_err() {
                    return;
                }
            }
        });
    }

    pub fn collect(&mut self) -> bool {
        let mut changed = false;
        while let Ok((id, r)) = self.rx.try_recv() {
            self.in_flight = self.in_flight.saturating_sub(1);
            match r {
                Ok(prs) => {
                    self.errors.remove(&id);
                    changed |= self.prs.get(&id) != Some(&prs);
                    self.prs.insert(id, prs);
                }
                Err(e) => {
                    self.errors.insert(id, e);
                }
            }
        }
        changed
    }

    pub fn pr_for(&self, project: &str, branch: &str) -> Option<&Pr> {
        self.prs.get(project)?.iter().find(|p| p.head == branch)
    }
}

type ActionMsg = (String, Action, Result<String, String>);

/// Uma ação de entrega por vez, em segundo plano.
pub struct Runner {
    tx: Sender<ActionMsg>,
    rx: Receiver<ActionMsg>,
    pub running: Option<(String, Action)>,
}

impl Default for Runner {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            tx,
            rx,
            running: None,
        }
    }
}

impl Runner {
    pub fn start(
        &mut self,
        gh: &str,
        session: String,
        cwd: PathBuf,
        branch: String,
        base: String,
        action: Action,
    ) {
        self.running = Some((session.clone(), action.clone()));
        let tx = self.tx.clone();
        let gh = gh.to_string();
        std::thread::spawn(move || {
            let r = execute(&gh, &cwd, &branch, &base, &action).map_err(|e| format!("{e:#}"));
            let _ = tx.send((session, action, r));
        });
    }

    pub fn collect(&mut self) -> Option<ActionMsg> {
        let msg = self.rx.try_recv().ok()?;
        self.running = None;
        Some(msg)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn resumo_do_ci() {
        assert_eq!(ci_of(&[]), Ci::NoChecks);
        let ok = json!({"__typename":"CheckRun","status":"COMPLETED","conclusion":"SUCCESS"});
        let skip = json!({"__typename":"CheckRun","status":"COMPLETED","conclusion":"SKIPPED"});
        let run = json!({"__typename":"CheckRun","status":"IN_PROGRESS","conclusion":""});
        let fail = json!({"__typename":"CheckRun","status":"COMPLETED","conclusion":"FAILURE"});
        let ctx_pend = json!({"__typename":"StatusContext","state":"PENDING"});
        let ctx_err = json!({"__typename":"StatusContext","state":"ERROR"});
        let weird = json!({"foo":"bar"});
        assert_eq!(ci_of(&[ok.clone(), skip.clone(), weird.clone()]), Ci::Ok);
        assert_eq!(ci_of(&[ok.clone(), run.clone(), ctx_pend]), Ci::Pending(2));
        assert_eq!(ci_of(&[ok, run, fail, ctx_err]), Ci::Failing(2));
        assert_eq!(ci_of(&[weird]), Ci::NoChecks);
    }

    #[test]
    fn ci_com_formato_real_do_github() {
        // Capturado do gh 2.97 no PR de teste (ADR-0017, tarefa 4.3).
        let rodando = r#"[{"number":1,"headRefName":"ccdeck/entrega-real","statusCheckRollup":[
            {"__typename":"CheckRun","name":"passa","status":"IN_PROGRESS","conclusion":""},
            {"__typename":"CheckRun","name":"falha","status":"IN_PROGRESS","conclusion":""},
            {"__typename":"CheckRun","name":"GitGuardian Security Checks","status":"COMPLETED","conclusion":"SUCCESS"}]}]"#;
        let fim = r#"[{"number":1,"headRefName":"ccdeck/entrega-real","statusCheckRollup":[
            {"__typename":"CheckRun","name":"passa","status":"COMPLETED","conclusion":"SUCCESS"},
            {"__typename":"CheckRun","name":"falha","status":"COMPLETED","conclusion":"FAILURE"},
            {"__typename":"CheckRun","name":"GitGuardian Security Checks","status":"COMPLETED","conclusion":"SUCCESS"}]}]"#;
        assert_eq!(parse_prs(rodando).unwrap()[0].ci, Ci::Pending(2));
        assert_eq!(parse_prs(fim).unwrap()[0].ci, Ci::Failing(1));
    }

    #[test]
    fn parse_da_lista_de_prs() {
        let j = r#"[{"number":12,"url":"https://github.com/o/r/pull/12","headRefName":"ccdeck/add-x","title":"Add X",
                     "statusCheckRollup":[{"status":"COMPLETED","conclusion":"SUCCESS"}]},
                    {"number":13,"headRefName":"outra"},
                    {"sem":"numero"}]"#;
        let prs = parse_prs(j).unwrap();
        assert_eq!(prs.len(), 2);
        assert_eq!((prs[0].number, prs[0].ci.clone()), (12, Ci::Ok));
        assert_eq!(prs[1].ci, Ci::NoChecks);
        assert!(parse_prs("não json").is_err());
    }

    #[test]
    fn commit_e_push_num_remoto_local() {
        let root = std::env::temp_dir().join(format!("ccdeck-dl-{}", crate::model::short_id()));
        let remote = root.join("remoto.git");
        let work = root.join("work");
        std::fs::create_dir_all(&work).unwrap();
        let g = |dir: &Path, args: &[&str]| run(dir, "git", args).unwrap();
        g(
            &root,
            &[
                "init",
                "-q",
                "--bare",
                "-b",
                "main",
                remote.to_str().unwrap(),
            ],
        );
        g(&work, &["init", "-q", "-b", "main"]);
        g(&work, &["config", "user.email", "t@t"]);
        g(&work, &["config", "user.name", "t"]);
        g(
            &work,
            &["remote", "add", "origin", remote.to_str().unwrap()],
        );
        g(&work, &["commit", "-q", "--allow-empty", "-m", "init"]);
        g(&work, &["checkout", "-q", "-b", "ccdeck/x"]);
        std::fs::write(work.join("a.txt"), "x").unwrap();
        let c = Action::Commit {
            message: "feito".into(),
        };
        execute("gh", &work, "ccdeck/x", "main", &c).unwrap();
        execute("gh", &work, "ccdeck/x", "main", &Action::Push).unwrap();
        let log = g(&remote, &["log", "--oneline", "ccdeck/x"]);
        assert!(log.contains("feito"), "{log}");
        // Commit sem nada para commitar: o erro do git volta inteiro.
        let e = execute("gh", &work, "ccdeck/x", "main", &c).unwrap_err();
        assert!(format!("{e:#}").contains("git commit"), "{e:#}");
        std::fs::remove_dir_all(root).ok();
    }
}
