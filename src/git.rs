//! Operações git via CLI (ADR-0003).

use std::path::{Path, PathBuf};
use std::process::Command;

use anyhow::{Context, Result, bail};

fn git(dir: &Path, args: &[&str]) -> Result<String> {
    let out = Command::new("git")
        .arg("-C")
        .arg(dir)
        .args(args)
        .output()
        .context("falha ao executar git (está instalado?)")?;
    if !out.status.success() {
        bail!(
            "git {}: {}",
            args.join(" "),
            String::from_utf8_lossy(&out.stderr).trim()
        );
    }
    Ok(String::from_utf8_lossy(&out.stdout).trim().to_string())
}

/// Raiz do repositório que contém `dir`, se houver.
pub fn repo_root(dir: &Path) -> Option<PathBuf> {
    git(dir, &["rev-parse", "--show-toplevel"])
        .ok()
        .map(PathBuf::from)
}

pub fn current_branch(dir: &Path) -> Option<String> {
    git(dir, &["rev-parse", "--abbrev-ref", "HEAD"])
        .ok()
        .filter(|b| b != "HEAD")
}

/// Se `path` existe em `rev` (ex.: change do OpenSpec já commitado na base).
pub fn exists_in_ref(repo: &Path, rev: &str, path: &str) -> bool {
    git(repo, &["cat-file", "-e", &format!("{rev}:{path}")]).is_ok()
}

/// `<repo>.worktrees/<slug>` — irmão do repositório (ADR-0003).
pub fn worktree_path(repo: &Path, slug: &str) -> PathBuf {
    let name = repo
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "repo".into());
    repo.with_file_name(format!("{name}.worktrees")).join(slug)
}

pub fn worktree_add(repo: &Path, path: &Path, branch: &str, base: &str) -> Result<()> {
    if path.exists() {
        bail!("o diretório {} já existe", path.display());
    }
    git(
        repo,
        &[
            "worktree",
            "add",
            "-b",
            branch,
            &path.to_string_lossy(),
            base,
        ],
    )?;
    Ok(())
}

pub fn is_dirty(dir: &Path) -> Result<bool> {
    Ok(!git(dir, &["status", "--porcelain"])?.is_empty())
}

pub fn dirty_count(dir: &Path) -> usize {
    git(dir, &["status", "--porcelain"])
        .map(|s| s.lines().filter(|l| !l.is_empty()).count())
        .unwrap_or(0)
}

pub fn worktree_remove(repo: &Path, path: &Path, force: bool) -> Result<()> {
    if !force && is_dirty(path)? {
        bail!("há alterações não commitadas em {}", path.display());
    }
    let p = path.to_string_lossy();
    let mut args = vec!["worktree", "remove"];
    if force {
        args.push("--force");
    }
    args.push(&p);
    git(repo, &args)?;
    Ok(())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangeKind {
    Modified,
    Added,
    Deleted,
    Untracked,
}

impl ChangeKind {
    pub fn mark(self) -> &'static str {
        match self {
            ChangeKind::Modified => "M",
            ChangeKind::Added => "A",
            ChangeKind::Deleted => "D",
            ChangeKind::Untracked => "?",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileChange {
    /// Relativo à raiz do worktree.
    pub path: String,
    /// `None` em arquivos binários.
    pub added: Option<u32>,
    pub deleted: Option<u32>,
    pub kind: ChangeKind,
}

/// Ponto de comparação: merge-base com a base ou HEAD (ADR-0009).
pub fn diff_base(dir: &Path, base: Option<&str>) -> String {
    base.and_then(|b| git(dir, &["merge-base", "HEAD", b]).ok())
        .unwrap_or_else(|| "HEAD".into())
}

/// Arquivos alterados desde a base: commits do branch + não commitados + não rastreados.
pub fn changes(dir: &Path, base: Option<&str>) -> Result<Vec<FileChange>> {
    let base_ref = diff_base(dir, base);
    let mut kinds = std::collections::HashMap::new();
    for line in git(dir, &["diff", "--no-renames", "--name-status", &base_ref])?.lines() {
        if let Some((k, path)) = line.split_once('\t') {
            let kind = match k.chars().next() {
                Some('A') => ChangeKind::Added,
                Some('D') => ChangeKind::Deleted,
                _ => ChangeKind::Modified,
            };
            kinds.insert(path.to_string(), kind);
        }
    }
    let mut out = Vec::new();
    for line in git(dir, &["diff", "--no-renames", "--numstat", &base_ref])?.lines() {
        let mut parts = line.splitn(3, '\t');
        let (Some(a), Some(d), Some(path)) = (parts.next(), parts.next(), parts.next()) else {
            continue;
        };
        out.push(FileChange {
            path: path.to_string(),
            added: a.parse().ok(),
            deleted: d.parse().ok(),
            kind: kinds.get(path).copied().unwrap_or(ChangeKind::Modified),
        });
    }
    for path in git(dir, &["ls-files", "--others", "--exclude-standard"])?.lines() {
        if path.is_empty() {
            continue;
        }
        let lines = std::fs::read(dir.join(path))
            .ok()
            .filter(|b| b.len() < 2_000_000 && !b.contains(&0))
            .map(|b| b.iter().filter(|&&c| c == b'\n').count() as u32);
        out.push(FileChange {
            path: path.to_string(),
            added: lines,
            deleted: Some(0),
            kind: ChangeKind::Untracked,
        });
    }
    out.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn caminho_do_worktree() {
        assert_eq!(
            worktree_path(Path::new("/home/e/dev/vendax"), "fix-login"),
            PathBuf::from("/home/e/dev/vendax.worktrees/fix-login")
        );
    }

    #[test]
    fn ciclo_de_worktree() {
        let root = std::env::temp_dir().join(format!("ccdeck-git-{}", crate::model::short_id()));
        let repo = root.join("repo");
        std::fs::create_dir_all(&repo).unwrap();
        let run = |args: &[&str]| {
            let ok = Command::new("git")
                .arg("-C")
                .arg(&repo)
                .args(args)
                .output()
                .unwrap();
            assert!(
                ok.status.success(),
                "{:?}",
                String::from_utf8_lossy(&ok.stderr)
            );
        };
        run(&["init", "-q", "-b", "main"]);
        run(&[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "--allow-empty",
            "-m",
            "init",
        ]);
        assert_eq!(
            repo_root(&repo).unwrap().canonicalize().unwrap(),
            repo.canonicalize().unwrap()
        );
        assert_eq!(current_branch(&repo).as_deref(), Some("main"));

        let wt = worktree_path(&repo, "teste");
        worktree_add(&repo, &wt, "ccdeck/teste", "main").unwrap();
        assert_eq!(current_branch(&wt).as_deref(), Some("ccdeck/teste"));

        std::fs::write(wt.join("novo.txt"), "x").unwrap();
        // Alterações: um commit no branch + um arquivo novo não rastreado.
        std::fs::write(wt.join("a.txt"), "1\n2\n").unwrap();
        let g = |args: &[&str]| {
            assert!(
                Command::new("git")
                    .arg("-C")
                    .arg(&wt)
                    .args(args)
                    .output()
                    .unwrap()
                    .status
                    .success()
            )
        };
        g(&["add", "a.txt"]);
        g(&[
            "-c",
            "user.email=t@t",
            "-c",
            "user.name=t",
            "commit",
            "-q",
            "-m",
            "a",
        ]);
        let ch = changes(&wt, Some("main")).unwrap();
        assert_eq!(ch.len(), 2, "{ch:?}");
        assert_eq!(ch[0].path, "a.txt");
        assert_eq!(ch[0].kind, ChangeKind::Added);
        assert_eq!(ch[0].added, Some(2));
        assert_eq!(ch[1].path, "novo.txt");
        assert_eq!(ch[1].kind, ChangeKind::Untracked);

        assert!(worktree_remove(&repo, &wt, false).is_err());
        worktree_remove(&repo, &wt, true).unwrap();
        assert!(!wt.exists());
        std::fs::remove_dir_all(root).ok();
    }
}
