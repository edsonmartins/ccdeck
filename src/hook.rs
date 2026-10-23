//! Subcomando `ccdeck hook` e geração do settings com hooks (ADR-0004).
//!
//! Regra de ouro: o hook nunca pode atrapalhar o Claude Code. Qualquer erro é
//! engolido e o processo sai com código 0.

use std::io::Read;
use std::path::Path;

use anyhow::Result;
use chrono::Utc;
use serde_json::{Value, json};

use crate::model::{Status, StatusRecord, ToolCall};
use crate::paths::{Paths, write_atomic};

/// Eventos do Claude Code que o ccdeck escuta.
pub const EVENTS: [&str; 9] = [
    "SessionStart",
    "UserPromptSubmit",
    "PreToolUse",
    "PostToolUse",
    "PermissionRequest",
    "Notification",
    "Stop",
    "StopFailure",
    "SessionEnd",
];

/// Traduz um evento de hook num estado. `None` = evento não muda o estado.
pub fn map_event(input: &Value) -> Option<(Status, String, Option<String>)> {
    let event = input.get("hook_event_name")?.as_str()?.to_string();
    let tool = input
        .get("tool_name")
        .and_then(Value::as_str)
        .map(str::to_string);
    let status = match event.as_str() {
        "SessionStart" => Status::Yours,
        "UserPromptSubmit" | "PreToolUse" | "PostToolUse" => Status::Working,
        "PermissionRequest" => Status::Approval,
        "Stop" => Status::Yours,
        "StopFailure" => Status::Error,
        "SessionEnd" => Status::Ended,
        "Notification" => {
            let kind = input
                .get("notification_type")
                .and_then(Value::as_str)
                .unwrap_or("");
            let status = match kind {
                "permission_prompt" => Status::Approval,
                "idle_prompt"
                | "agent_needs_input"
                | "elicitation_dialog"
                | "elicitation_url_dialog" => Status::Yours,
                _ => return None,
            };
            let msg = input
                .get("message")
                .and_then(Value::as_str)
                .map(str::to_string);
            return Some((status, format!("Notification · {kind}"), msg));
        }
        _ => return None,
    };
    let label = match &tool {
        Some(t) if event.ends_with("ToolUse") || event == "PermissionRequest" => {
            format!("{event} · {t}")
        }
        _ => event.clone(),
    };
    Some((status, label, None))
}

/// Ponto de entrada de `ccdeck hook`.
pub fn run(paths: &Paths) {
    let _ = run_inner(paths);
}

fn run_inner(paths: &Paths) -> Result<()> {
    let Some(sid) = std::env::var("CCDECK_SID").ok().filter(|s| !s.is_empty()) else {
        // Sessão iniciada fora do ccdeck: nada a fazer.
        return Ok(());
    };
    let mut buf = String::new();
    std::io::stdin().read_to_string(&mut buf)?;
    let input: Value = serde_json::from_str(&buf)?;
    record(paths, &sid, &input)
}

pub fn record(paths: &Paths, sid: &str, input: &Value) -> Result<()> {
    let Some((status, event, detail)) = map_event(input) else {
        return Ok(());
    };
    let file = paths.status_file(sid);
    let prev: Option<StatusRecord> = std::fs::read(&file)
        .ok()
        .and_then(|b| serde_json::from_slice(&b).ok());
    let now = Utc::now();
    let rec = merge(prev.as_ref(), input, status, event, detail, now);
    write_atomic(&file, &serde_json::to_vec(&rec)?)
}

/// Monta o novo registro carregando adiante o que o evento não traz (ADR-0007).
pub fn merge(
    prev: Option<&StatusRecord>,
    input: &Value,
    status: Status,
    event: String,
    detail: Option<String>,
    now: chrono::DateTime<Utc>,
) -> StatusRecord {
    let str_field = |k: &str| input.get(k).and_then(Value::as_str).map(str::to_string);
    let this_tool = tool_call(input, now);
    let last_tool = this_tool
        .clone()
        .or_else(|| prev.and_then(|p| p.last_tool.clone()));
    let hook_event = str_field("hook_event_name").unwrap_or_default();
    let pending = match status {
        Status::Approval if hook_event == "PermissionRequest" => this_tool
            .clone()
            .or_else(|| prev.and_then(|p| p.last_tool.clone())),
        // Notification(permission_prompt) depois do PermissionRequest: mesmo pedido.
        Status::Approval => prev
            .and_then(|p| p.pending.clone())
            .or_else(|| prev.and_then(|p| p.last_tool.clone())),
        _ => None,
    };
    let last_message = match hook_event.as_str() {
        "Stop" => str_field("last_assistant_message")
            .filter(|m| !m.trim().is_empty())
            .or_else(|| prev.and_then(|p| p.last_message.clone())),
        _ => prev.and_then(|p| p.last_message.clone()),
    };
    StatusRecord {
        status,
        event,
        detail,
        claude_session_id: str_field("session_id")
            .or_else(|| prev.and_then(|p| p.claude_session_id.clone())),
        at: now,
        pending,
        last_tool,
        last_message,
        transcript_path: str_field("transcript_path")
            .or_else(|| prev.and_then(|p| p.transcript_path.clone())),
    }
}

/// Ferramenta + resumo legível do `tool_input`.
fn tool_call(input: &Value, now: chrono::DateTime<Utc>) -> Option<ToolCall> {
    let tool = input.get("tool_name")?.as_str()?.to_string();
    let ti = input.get("tool_input").cloned().unwrap_or(Value::Null);
    let s = |k: &str| ti.get(k).and_then(Value::as_str).map(str::to_string);
    let summary = match tool.as_str() {
        "Bash" => s("command"),
        "Edit" | "MultiEdit" | "Write" | "Read" | "NotebookEdit" => {
            s("file_path").or_else(|| s("notebook_path"))
        }
        "WebFetch" => s("url"),
        "WebSearch" => s("query"),
        "Grep" | "Glob" => s("pattern"),
        "Task" | "Agent" => s("description"),
        _ => None,
    }
    .unwrap_or_else(|| {
        if ti.is_null() {
            String::new()
        } else {
            ti.to_string()
        }
    });
    let id = input
        .get("tool_use_id")
        .and_then(Value::as_str)
        .map(str::to_string)
        .unwrap_or_else(|| format!("t{}", now.timestamp_nanos_opt().unwrap_or_default()));
    Some(ToolCall {
        tool,
        summary: clip(&summary, 600),
        id,
    })
}

fn clip(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        return s.to_string();
    }
    let mut out: String = s.chars().take(max).collect();
    out.push('…');
    out
}

/// Gera o arquivo passado via `claude --settings`. Idempotente.
pub fn write_settings(paths: &Paths, exe: &Path) -> Result<()> {
    let command = format!("{} hook", shell_quote(&exe.to_string_lossy()));
    let handler = json!([{ "hooks": [{ "type": "command", "command": command, "timeout": 5 }] }]);
    let mut hooks = serde_json::Map::new();
    for ev in EVENTS {
        hooks.insert(ev.to_string(), handler.clone());
    }
    let settings = json!({ "hooks": hooks });
    write_atomic(&paths.hooks_file(), &serde_json::to_vec_pretty(&settings)?)
}

/// Aspas simples POSIX.
pub fn shell_quote(s: &str) -> String {
    if !s.is_empty()
        && s.chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_./:=@,+".contains(c))
    {
        return s.to_string();
    }
    format!("'{}'", s.replace('\'', r"'\''"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn mapeia_eventos() {
        let st = |v: Value| map_event(&v).map(|x| x.0);
        assert_eq!(
            st(json!({"hook_event_name":"UserPromptSubmit"})),
            Some(Status::Working)
        );
        assert_eq!(st(json!({"hook_event_name":"Stop"})), Some(Status::Yours));
        assert_eq!(
            st(json!({"hook_event_name":"StopFailure"})),
            Some(Status::Error)
        );
        assert_eq!(
            st(json!({"hook_event_name":"Notification","notification_type":"permission_prompt"})),
            Some(Status::Approval)
        );
        assert_eq!(
            st(json!({"hook_event_name":"Notification","notification_type":"idle_prompt"})),
            Some(Status::Yours)
        );
        assert_eq!(
            st(json!({"hook_event_name":"Notification","notification_type":"auth_success"})),
            None
        );
        assert_eq!(st(json!({"hook_event_name":"PreCompact"})), None);
        let (_, label, _) =
            map_event(&json!({"hook_event_name":"PreToolUse","tool_name":"Bash"})).unwrap();
        assert_eq!(label, "PreToolUse · Bash");
    }

    #[test]
    fn grava_status_e_settings() {
        let dir = std::env::temp_dir().join(format!("ccdeck-test-{}", crate::model::short_id()));
        let paths = Paths::rooted(&dir);
        record(
            &paths,
            "abc",
            &json!({"hook_event_name":"Stop","session_id":"u-1"}),
        )
        .unwrap();
        let recs = crate::store::load_status_records(&paths);
        assert_eq!(recs["abc"].status, Status::Yours);
        assert_eq!(recs["abc"].claude_session_id.as_deref(), Some("u-1"));

        write_settings(&paths, Path::new("/opt/my apps/ccdeck")).unwrap();
        let v: Value = serde_json::from_slice(&std::fs::read(paths.hooks_file()).unwrap()).unwrap();
        assert_eq!(
            v["hooks"]["Stop"][0]["hooks"][0]["command"],
            "'/opt/my apps/ccdeck' hook"
        );
        assert_eq!(v["hooks"].as_object().unwrap().len(), EVENTS.len());
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn pedido_pendente_e_ultima_mensagem() {
        let dir = std::env::temp_dir().join(format!("ccdeck-hook-{}", crate::model::short_id()));
        let paths = Paths::rooted(&dir);
        let ev = |v: Value| record(&paths, "s", &v).unwrap();
        let get = || {
            crate::store::load_status_records(&paths)
                .remove("s")
                .unwrap()
        };

        ev(
            json!({"hook_event_name":"PreToolUse","tool_name":"Bash","tool_use_id":"tu1",
                  "tool_input":{"command":"./mvnw flyway:migrate","description":"migra"},
                  "transcript_path":"/tmp/t.jsonl"}),
        );
        assert_eq!(get().last_tool.unwrap().summary, "./mvnw flyway:migrate");
        assert!(get().pending.is_none());

        ev(
            json!({"hook_event_name":"PermissionRequest","tool_name":"Bash","tool_use_id":"tu1",
                  "tool_input":{"command":"./mvnw flyway:migrate"}}),
        );
        let r = get();
        assert_eq!(r.status, Status::Approval);
        assert_eq!(r.pending.as_ref().unwrap().id, "tu1");

        // A notificação que vem depois mantém o mesmo pedido.
        ev(
            json!({"hook_event_name":"Notification","notification_type":"permission_prompt",
                  "message":"Claude needs your permission to use Bash"}),
        );
        let r = get();
        assert_eq!(r.pending.as_ref().unwrap().id, "tu1");
        assert_eq!(r.transcript_path.as_deref(), Some("/tmp/t.jsonl"));

        ev(
            json!({"hook_event_name":"PostToolUse","tool_name":"Bash","tool_use_id":"tu1","tool_input":{}}),
        );
        assert!(get().pending.is_none());

        ev(json!({"hook_event_name":"Stop","last_assistant_message":"Pronto. Abro o PR?"}));
        assert_eq!(get().last_message.as_deref(), Some("Pronto. Abro o PR?"));
        ev(json!({"hook_event_name":"UserPromptSubmit"}));
        assert_eq!(get().last_message.as_deref(), Some("Pronto. Abro o PR?"));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn resumo_de_ferramentas() {
        let now = Utc::now();
        let t = |v: Value| tool_call(&v, now).unwrap().summary;
        assert_eq!(
            t(json!({"tool_name":"Edit","tool_input":{"file_path":"/a/B.java"}})),
            "/a/B.java"
        );
        assert_eq!(
            t(json!({"tool_name":"WebFetch","tool_input":{"url":"https://x.dev"}})),
            "https://x.dev"
        );
        assert_eq!(
            t(json!({"tool_name":"mcp__x__y","tool_input":{"a":1}})),
            r#"{"a":1}"#
        );
    }

    #[test]
    fn quote() {
        assert_eq!(shell_quote("/usr/bin/claude"), "/usr/bin/claude");
        assert_eq!(shell_quote("it's"), r"'it'\''s'");
    }
}
