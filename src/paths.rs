//! Caminhos de arquivos do ccdeck (SPEC §7, ADR-0005).
//!
//! `CCDECK_HOME` sobrescreve tudo e coloca config, dados e estado num único
//! diretório — usado em testes e por quem quiser portar a instalação.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use directories::ProjectDirs;

#[derive(Debug, Clone)]
pub struct Paths {
    pub config_file: PathBuf,
    pub state_file: PathBuf,
    pub state_dir: PathBuf,
}

impl Paths {
    pub fn resolve() -> Result<Self> {
        if let Some(home) = std::env::var_os("CCDECK_HOME") {
            return Ok(Self::rooted(Path::new(&home)));
        }
        let dirs = ProjectDirs::from("dev", "IntegrAllTech", "ccdeck")
            .context("não foi possível descobrir o diretório home")?;
        let state_dir = dirs
            .state_dir()
            .map(Path::to_path_buf)
            .unwrap_or_else(|| dirs.data_local_dir().join("state"));
        Ok(Self {
            config_file: dirs.config_dir().join("config.json"),
            state_file: dirs.data_dir().join("state.json"),
            state_dir,
        })
    }

    pub fn rooted(root: &Path) -> Self {
        Self {
            config_file: root.join("config.json"),
            state_file: root.join("state.json"),
            state_dir: root.join("state"),
        }
    }

    pub fn hooks_file(&self) -> PathBuf {
        self.state_dir.join("hooks.json")
    }

    pub fn tmux_conf(&self) -> PathBuf {
        self.state_dir.join("tmux.conf")
    }

    pub fn status_dir(&self) -> PathBuf {
        self.state_dir.join("status")
    }

    pub fn status_file(&self, session_id: &str) -> PathBuf {
        self.status_dir().join(format!("{session_id}.json"))
    }
}

/// Grava de forma atômica: escreve num temporário ao lado e renomeia.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> Result<()> {
    let dir = path.parent().context("caminho sem diretório pai")?;
    std::fs::create_dir_all(dir).with_context(|| format!("criando {}", dir.display()))?;
    let tmp = dir.join(format!(
        ".{}.{}.tmp",
        path.file_name()
            .and_then(|n| n.to_str())
            .unwrap_or("arquivo"),
        std::process::id()
    ));
    std::fs::write(&tmp, bytes).with_context(|| format!("gravando {}", tmp.display()))?;
    std::fs::rename(&tmp, path).with_context(|| format!("renomeando para {}", path.display()))?;
    Ok(())
}
