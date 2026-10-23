//! Persistência do estado e da configuração em JSON (ADR-0005).

use std::collections::HashMap;

use anyhow::{Context, Result, bail};

use crate::model::{Config, SCHEMA_VERSION, State, StatusRecord};
use crate::paths::{Paths, write_atomic};

pub fn load_state(paths: &Paths) -> Result<State> {
    let file = &paths.state_file;
    if !file.exists() {
        return Ok(State::default());
    }
    let raw = std::fs::read(file).with_context(|| format!("lendo {}", file.display()))?;
    let value: serde_json::Value = serde_json::from_slice(&raw)
        .with_context(|| format!("JSON inválido em {}", file.display()))?;
    let version = value
        .get("schema_version")
        .and_then(|v| v.as_u64())
        .unwrap_or(0) as u32;
    let value = migrate(value, version)?;
    Ok(serde_json::from_value(value)?)
}

/// Aplica migrações `vN -> vN+1` até a versão atual.
fn migrate(mut value: serde_json::Value, mut from: u32) -> Result<serde_json::Value> {
    if from > SCHEMA_VERSION {
        bail!(
            "state.json tem schema_version {from}, mais novo que este ccdeck ({SCHEMA_VERSION}); atualize o ccdeck"
        );
    }
    while from < SCHEMA_VERSION {
        value = match from {
            1 => v1_to_v2(value),
            v => bail!("schema_version {v} não suportado"),
        };
        from += 1;
    }
    Ok(value)
}

/// v2 (ADR-0008): sessões ganham `base`, `port_slot` e `ports`.
/// Sessões antigas com worktree assumem `main` como base.
fn v1_to_v2(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(sessions) = value.get_mut("sessions").and_then(|s| s.as_array_mut()) {
        for s in sessions {
            let worktree = s.get("worktree").and_then(|w| w.as_bool()).unwrap_or(false);
            if let Some(obj) = s.as_object_mut() {
                obj.entry("base").or_insert(if worktree {
                    serde_json::json!("main")
                } else {
                    serde_json::Value::Null
                });
                obj.entry("port_slot").or_insert(serde_json::Value::Null);
                obj.entry("ports").or_insert(serde_json::json!([]));
            }
        }
    }
    value["schema_version"] = serde_json::json!(2);
    value
}

pub fn save_state(paths: &Paths, state: &State) -> Result<()> {
    write_atomic(&paths.state_file, &serde_json::to_vec_pretty(state)?)
}

pub fn load_config(paths: &Paths) -> Result<Config> {
    let file = &paths.config_file;
    if !file.exists() {
        let cfg = Config::default();
        write_atomic(file, &serde_json::to_vec_pretty(&cfg)?)?;
        return Ok(cfg);
    }
    let raw = std::fs::read(file)?;
    serde_json::from_slice(&raw).with_context(|| format!("JSON inválido em {}", file.display()))
}

/// Lê todos os `status/<sid>.json`. Arquivos corrompidos são ignorados.
pub fn load_status_records(paths: &Paths) -> HashMap<String, StatusRecord> {
    let mut out = HashMap::new();
    let Ok(entries) = std::fs::read_dir(paths.status_dir()) else {
        return out;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let Some(sid) = path.file_stem().and_then(|s| s.to_str()) else {
            continue;
        };
        if let Ok(raw) = std::fs::read(&path)
            && let Ok(rec) = serde_json::from_slice::<StatusRecord>(&raw)
        {
            out.insert(sid.to_string(), rec);
        }
    }
    out
}

/// Lê o status de uma sessão direto do disco (usado nas travas do `y`/`x`).
pub fn load_status(paths: &Paths, sid: &str) -> Option<StatusRecord> {
    let raw = std::fs::read(paths.status_file(sid)).ok()?;
    serde_json::from_slice(&raw).ok()
}

pub fn remove_status(paths: &Paths, sid: &str) {
    let _ = std::fs::remove_file(paths.status_file(sid));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn migra_v1_para_v2() {
        let dir = std::env::temp_dir().join(format!("ccdeck-store-{}", crate::model::short_id()));
        let paths = Paths::rooted(&dir);
        let v1 = serde_json::json!({
            "schema_version": 1,
            "projects": [{"id": "p1", "name": "repo", "path": "/tmp/repo"}],
            "sessions": [{
                "id": "s1", "project_id": "p1", "name": "x", "cwd": "/tmp/repo.worktrees/x",
                "branch": "ccdeck/x", "worktree": true,
                "claude_session_id": "20849939-a3d7-4fef-9319-fdbd38703902",
                "permission_mode": "default", "notes": "", "created_at": "2026-10-23T22:00:00Z",
                "started_once": true
            }]
        });
        write_atomic(&paths.state_file, v1.to_string().as_bytes()).unwrap();
        let st = load_state(&paths).unwrap();
        assert_eq!(st.schema_version, 2);
        assert_eq!(st.sessions[0].base.as_deref(), Some("main"));
        assert!(st.sessions[0].ports.is_empty());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn recusa_schema_futuro() {
        assert!(migrate(serde_json::json!({}), 99).is_err());
    }
}
