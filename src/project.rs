//! Configuração de projeto `.ccdeck.json` e preparo do worktree (ADR-0008).

use std::collections::BTreeMap;
use std::path::{Component, Path, PathBuf};

use anyhow::{Context, Result, bail};
use serde::{Deserialize, Serialize};

use crate::model::PortVar;

pub const FILE: &str = ".ccdeck.json";

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq)]
#[serde(default)]
pub struct ProjectConfig {
    pub copy: Vec<String>,
    pub setup: Option<String>,
    /// Comando de verificação rodado com `t` (ADR-0013).
    pub verify: Option<String>,
    /// Modelos de prompt para novas sessões (ADR-0014).
    pub templates: Vec<Template>,
    /// Prompt para sessão a partir de change do OpenSpec; `{id}` e `{dir}`.
    pub openspec_prompt: Option<String>,
    pub env: BTreeMap<String, String>,
    pub ports: Option<Ports>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Template {
    pub name: String,
    pub prompt: String,
    /// Modo de permissão (`default`, `acceptEdits`, `plan`, `auto`).
    #[serde(default)]
    pub mode: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq)]
pub struct Ports {
    pub base: u16,
    #[serde(default = "default_step")]
    pub step: u16,
    pub vars: Vec<String>,
}

fn default_step() -> u16 {
    10
}

impl ProjectConfig {
    /// Lê `<repo>/.ccdeck.json`. Ausente = configuração vazia.
    pub fn load(repo: &Path) -> Result<Self> {
        let path = repo.join(FILE);
        if !path.exists() {
            return Ok(Self::default());
        }
        let raw = std::fs::read(&path)?;
        serde_json::from_slice(&raw).with_context(|| format!("JSON inválido em {}", path.display()))
    }

    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Portas de um slot: `base + slot*step + i`.
    pub fn ports_for(&self, slot: u32) -> Vec<PortVar> {
        let Some(p) = &self.ports else {
            return Vec::new();
        };
        p.vars
            .iter()
            .enumerate()
            .filter_map(|(i, name)| {
                let port = u32::from(p.base) + slot * u32::from(p.step) + i as u32;
                u16::try_from(port).ok().map(|port| PortVar {
                    name: name.clone(),
                    port,
                })
            })
            .collect()
    }
}

/// Menor slot ≥ 1 que não está em uso.
pub fn free_slot(used: &[u32]) -> u32 {
    (1..).find(|n| !used.contains(n)).unwrap_or(1)
}

/// Caminho relativo seguro (sem absoluto, sem `..`).
pub fn safe_relative(p: &str) -> Option<PathBuf> {
    let path = Path::new(p);
    if p.trim().is_empty() || path.is_absolute() {
        return None;
    }
    path.components()
        .all(|c| matches!(c, Component::Normal(_) | Component::CurDir))
        .then(|| path.to_path_buf())
}

/// Resultado da cópia: (copiados, avisos).
pub fn copy_files(repo: &Path, worktree: &Path, items: &[String]) -> (Vec<String>, Vec<String>) {
    let mut copied = Vec::new();
    let mut warnings = Vec::new();
    for item in items {
        let Some(rel) = safe_relative(item) else {
            warnings.push(format!("ignorado (caminho inseguro): {item}"));
            continue;
        };
        let src = repo.join(&rel);
        let dst = worktree.join(&rel);
        if !src.is_file() {
            warnings.push(format!("não encontrado no repositório: {item}"));
            continue;
        }
        if dst.exists() {
            continue;
        }
        let res = dst
            .parent()
            .map(std::fs::create_dir_all)
            .transpose()
            .and_then(|_| std::fs::copy(&src, &dst));
        match res {
            Ok(_) => copied.push(item.clone()),
            Err(e) => warnings.push(format!("falha ao copiar {item}: {e}")),
        }
    }
    (copied, warnings)
}

pub const TEMPLATE: &str = r#"{
  "copy": [".env"],
  "setup": "",
  "verify": "",
  "templates": [
    { "name": "revisar", "prompt": "Revise as mudanças deste branch contra a base e aponte problemas antes de eu abrir o PR.", "mode": "plan" }
  ],
  "env": {},
  "ports": { "base": 8080, "step": 10, "vars": ["PORT"] }
}
"#;

/// `ccdeck init-project`: grava o exemplo se ainda não existir.
pub fn init(dir: &Path) -> Result<PathBuf> {
    let path = dir.join(FILE);
    if path.exists() {
        bail!("{} já existe", path.display());
    }
    std::fs::write(&path, TEMPLATE)?;
    Ok(path)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn portas_por_slot() {
        let cfg: ProjectConfig = serde_json::from_str(
            r#"{"ports":{"base":8080,"step":10,"vars":["PORT","SERVER_PORT"]}}"#,
        )
        .unwrap();
        let p = cfg.ports_for(2);
        assert_eq!(
            p[0],
            PortVar {
                name: "PORT".into(),
                port: 8100
            }
        );
        assert_eq!(
            p[1],
            PortVar {
                name: "SERVER_PORT".into(),
                port: 8101
            }
        );
        assert_eq!(free_slot(&[1, 3]), 2);
        assert_eq!(free_slot(&[]), 1);
        let t: ProjectConfig = serde_json::from_str(TEMPLATE).unwrap();
        assert_eq!(t.ports_for(1)[0].port, 8090);
    }

    #[test]
    fn caminhos_seguros() {
        assert!(safe_relative(".env").is_some());
        assert!(safe_relative("config/app.yml").is_some());
        assert!(safe_relative("../segredo").is_none());
        assert!(safe_relative("/etc/passwd").is_none());
        assert!(safe_relative("a/../../b").is_none());
    }

    #[test]
    fn copia_arquivos() {
        let root = std::env::temp_dir().join(format!("ccdeck-proj-{}", crate::model::short_id()));
        let (repo, wt) = (root.join("repo"), root.join("wt"));
        std::fs::create_dir_all(repo.join("config")).unwrap();
        std::fs::create_dir_all(&wt).unwrap();
        std::fs::write(repo.join(".env"), "A=1").unwrap();
        std::fs::write(repo.join("config/local.yml"), "x").unwrap();
        let (ok, warn) = copy_files(
            &repo,
            &wt,
            &[
                ".env".into(),
                "config/local.yml".into(),
                "faltando".into(),
                "../x".into(),
            ],
        );
        assert_eq!(ok, vec![".env", "config/local.yml"]);
        assert_eq!(warn.len(), 2);
        assert_eq!(std::fs::read_to_string(wt.join(".env")).unwrap(), "A=1");
        std::fs::remove_dir_all(root).ok();
    }
}
