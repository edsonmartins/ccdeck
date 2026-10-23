//! Leitura do transcript JSONL do Claude Code (ADR-0007).
//!
//! Lemos só o fim do arquivo: transcripts longos passam de dezenas de MB.

use std::io::{Read, Seek, SeekFrom};
use std::path::Path;

use serde_json::Value;

const TAIL_BYTES: u64 = 256 * 1024;

/// Último texto do assistente no transcript, se houver.
pub fn last_assistant_text(path: &Path) -> Option<String> {
    let mut f = std::fs::File::open(path).ok()?;
    let len = f.metadata().ok()?.len();
    let start = len.saturating_sub(TAIL_BYTES);
    f.seek(SeekFrom::Start(start)).ok()?;
    let mut buf = Vec::new();
    f.read_to_end(&mut buf).ok()?;
    let text = String::from_utf8_lossy(&buf);
    let mut lines = text.lines();
    if start > 0 {
        lines.next(); // primeira linha provavelmente cortada
    }
    lines
        .collect::<Vec<_>>()
        .into_iter()
        .rev()
        .find_map(assistant_text)
}

fn assistant_text(line: &str) -> Option<String> {
    let v: Value = serde_json::from_str(line).ok()?;
    let is_assistant = v.get("type").and_then(Value::as_str) == Some("assistant")
        || v.pointer("/message/role").and_then(Value::as_str) == Some("assistant");
    if !is_assistant {
        return None;
    }
    let content = v.pointer("/message/content")?;
    let text = match content {
        Value::String(s) => s.clone(),
        Value::Array(parts) => parts
            .iter()
            .filter(|p| p.get("type").and_then(Value::as_str) == Some("text"))
            .filter_map(|p| p.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    let text = text.trim().to_string();
    (!text.is_empty()).then_some(text)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acha_ultimo_texto_ignorando_tool_use() {
        let p = std::env::temp_dir().join(format!("ccdeck-tr-{}.jsonl", crate::model::short_id()));
        let lines = [
            r#"{"type":"user","message":{"role":"user","content":"oi"}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Primeira resposta"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"text","text":"Pronto."},{"type":"text","text":"Abro o PR?"}]}}"#,
            r#"{"type":"assistant","message":{"role":"assistant","content":[{"type":"tool_use","name":"Bash","input":{}}]}}"#,
            r#"{"type":"user","message":{"role":"user","content":[{"type":"tool_result","content":"ok"}]}}"#,
            "linha quebrada {",
        ];
        std::fs::write(&p, lines.join("\n")).unwrap();
        assert_eq!(
            last_assistant_text(&p).as_deref(),
            Some("Pronto.\nAbro o PR?")
        );
        std::fs::remove_file(p).ok();
    }
}
