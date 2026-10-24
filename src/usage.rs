//! Uso de tokens por sessão, lido dos transcripts do Claude Code (ADR-0010).
//!
//! O formato do transcript não é contrato público: o parser ignora o que não
//! entende e nunca falha a TUI. Leitura incremental por offset, porque um
//! transcript passa de gigabytes.

use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{Receiver, Sender, channel};

use anyhow::Result;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::Price;
use crate::paths::{Paths, write_atomic};

/// Versão do formato de `state/usage/<sid>.json`. Versão diferente = reindexar.
/// v2 (ADR-0012): contexto e compactações.
pub const FORMAT_VERSION: u32 = 2;

/// Prompt acima disso leva o sufixo `+long` no modelo: o Haiku 5.5 cobra mais
/// por requisição com prompt longo (entrada + escrita + leitura de cache).
pub const LONG_PROMPT: u64 = 100_000;

/// Modelo que o Claude Code usa para mensagens geradas por ele mesmo.
const SYNTHETIC: &str = "<synthetic>";

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Tokens {
    pub input: u64,
    pub output: u64,
    pub cache_write_5m: u64,
    pub cache_write_1h: u64,
    pub cache_read: u64,
    pub messages: u64,
}

impl Tokens {
    pub fn add(&mut self, o: &Tokens) {
        self.input += o.input;
        self.output += o.output;
        self.cache_write_5m += o.cache_write_5m;
        self.cache_write_1h += o.cache_write_1h;
        self.cache_read += o.cache_read;
        self.messages += o.messages;
    }

    fn sub(&mut self, o: &Tokens) {
        self.input = self.input.saturating_sub(o.input);
        self.output = self.output.saturating_sub(o.output);
        self.cache_write_5m = self.cache_write_5m.saturating_sub(o.cache_write_5m);
        self.cache_write_1h = self.cache_write_1h.saturating_sub(o.cache_write_1h);
        self.cache_read = self.cache_read.saturating_sub(o.cache_read);
        self.messages = self.messages.saturating_sub(o.messages);
    }

    pub fn cache_write(&self) -> u64 {
        self.cache_write_5m + self.cache_write_1h
    }

    /// Tudo que entrou no modelo: entrada + escrita + leitura de cache.
    pub fn prompt(&self) -> u64 {
        self.input + self.cache_write() + self.cache_read
    }

    pub fn is_empty(&self) -> bool {
        *self == Tokens::default()
    }
}

/// Uma mensagem do assistente já contada; a última de cada arquivo fica
/// guardada para trocar a contribuição quando chegam mais linhas dela.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Counted {
    pub id: String,
    pub hour: String,
    pub model: String,
    pub tokens: Tokens,
}

#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Cursor {
    pub offset: u64,
    #[serde(default)]
    pub last: Option<Counted>,
}

/// Contexto da última mensagem do transcript principal (ADR-0012).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ContextInfo {
    pub model: String,
    pub prompt: u64,
}

/// Linha `compact_boundary` do transcript principal.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Compaction {
    pub at: DateTime<Utc>,
    pub trigger: String,
    pub pre: u64,
    pub post: u64,
}

/// O que o painel e a linha do tempo mostram além dos tokens.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct Insight {
    #[serde(default)]
    pub context: Option<ContextInfo>,
    #[serde(default)]
    pub compactions: Vec<Compaction>,
}

/// Hora UTC (`AAAA-MM-DDTHH`) → modelo → tokens.
pub type Hours = BTreeMap<String, BTreeMap<String, Tokens>>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UsageIndex {
    pub version: u32,
    /// Transcript principal; permite reindexar depois que o status sumiu.
    #[serde(default)]
    pub transcript: Option<String>,
    /// Arquivo (principal ou subagente) → até onde já foi lido.
    pub files: BTreeMap<String, Cursor>,
    pub hours: Hours,
    #[serde(default)]
    pub insight: Insight,
}

impl Default for UsageIndex {
    fn default() -> Self {
        Self {
            version: FORMAT_VERSION,
            transcript: None,
            files: BTreeMap::new(),
            hours: BTreeMap::new(),
            insight: Insight::default(),
        }
    }
}

/// Uso de uma linha do transcript, se for mensagem do assistente com `usage`.
pub fn parse_line(line: &str) -> Option<Counted> {
    // Filtro barato antes do JSON: transcripts têm milhões de linhas.
    if !line.contains("\"assistant\"") {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("type")?.as_str()? != "assistant" {
        return None;
    }
    let msg = v.get("message")?;
    let model = msg.get("model")?.as_str()?;
    if model == SYNTHETIC {
        return None;
    }
    let id = msg.get("id")?.as_str()?.to_string();
    let u = msg.get("usage")?;
    let n = |k: &str| u.get(k).and_then(Value::as_u64).unwrap_or(0);
    let write_total = n("cache_creation_input_tokens");
    let (w5, w1) = match u.get("cache_creation") {
        Some(c) => {
            let g = |k: &str| c.get(k).and_then(Value::as_u64).unwrap_or(0);
            (
                g("ephemeral_5m_input_tokens"),
                g("ephemeral_1h_input_tokens"),
            )
        }
        // Sem detalhamento: o padrão do cache é 5 min.
        _ => (write_total, 0),
    };
    let tokens = Tokens {
        input: n("input_tokens"),
        output: n("output_tokens"),
        cache_write_5m: w5,
        cache_write_1h: w1,
        cache_read: n("cache_read_input_tokens"),
        messages: 1,
    };
    let ts: DateTime<Utc> = v.get("timestamp")?.as_str()?.parse().ok()?;
    let mut key = model.to_string();
    if u.get("speed").and_then(Value::as_str) == Some("fast") {
        key.push_str("+fast");
    }
    if tokens.prompt() > LONG_PROMPT {
        key.push_str("+long");
    }
    Some(Counted {
        id,
        hour: ts.format("%Y-%m-%dT%H").to_string(),
        model: key,
        tokens,
    })
}

/// Compactação registrada pelo Claude Code, se a linha for uma.
pub fn parse_compact(line: &str) -> Option<Compaction> {
    if !line.contains("compact_boundary") {
        return None;
    }
    let v: Value = serde_json::from_str(line).ok()?;
    if v.get("subtype")?.as_str()? != "compact_boundary" {
        return None;
    }
    let m = v.get("compactMetadata")?;
    let n = |k: &str| m.get(k).and_then(Value::as_u64).unwrap_or(0);
    Some(Compaction {
        at: v.get("timestamp")?.as_str()?.parse().ok()?,
        trigger: m
            .get("trigger")
            .and_then(Value::as_str)
            .unwrap_or("")
            .to_string(),
        pre: n("preTokens"),
        post: n("postTokens"),
    })
}

fn apply(hours: &mut Hours, c: &Counted, sign_add: bool) {
    let slot = hours
        .entry(c.hour.clone())
        .or_default()
        .entry(c.model.clone())
        .or_default();
    if sign_add {
        slot.add(&c.tokens);
    } else {
        slot.sub(&c.tokens);
        if slot.is_empty()
            && let Some(h) = hours.get_mut(&c.hour)
        {
            h.remove(&c.model);
            if h.is_empty() {
                hours.remove(&c.hour);
            }
        }
    }
}

/// Lê o que cresceu em `file` desde o cursor. Para na última linha completa.
/// Retorna se algo mudou.
/// `insight` só é preenchido para o transcript principal.
fn read_file(
    hours: &mut Hours,
    cursor: &mut Cursor,
    file: &Path,
    mut insight: Option<&mut Insight>,
) -> Result<bool> {
    let f = std::fs::File::open(file)?;
    let len = f.metadata()?.len();
    if len <= cursor.offset {
        return Ok(false);
    }
    let mut r = BufReader::with_capacity(1 << 20, f);
    r.seek(SeekFrom::Start(cursor.offset))?;
    let mut buf = Vec::new();
    let mut changed = false;
    loop {
        buf.clear();
        let n = r.read_until(b'\n', &mut buf)?;
        if n == 0 || buf.last() != Some(&b'\n') {
            break; // fim, ou linha ainda sendo escrita
        }
        cursor.offset += n as u64;
        let line = String::from_utf8_lossy(&buf);
        if let Some(ins) = insight.as_deref_mut()
            && let Some(comp) = parse_compact(&line)
        {
            ins.compactions.push(comp);
            changed = true;
            continue;
        }
        let Some(c) = parse_line(&line) else {
            continue;
        };
        if let Some(ins) = insight.as_deref_mut() {
            ins.context = Some(ContextInfo {
                model: display_model(&c.model).to_string(),
                prompt: c.tokens.prompt(),
            });
        }
        // Mais uma linha da mesma mensagem: a última vence.
        if let Some(prev) = cursor.last.take()
            && prev.id == c.id
        {
            apply(hours, &prev, false);
        }
        apply(hours, &c, true);
        cursor.last = Some(c);
        changed = true;
    }
    Ok(changed)
}

/// Transcript principal e os dos subagentes (`<dir>/<session_id>/subagents/*.jsonl`).
pub fn transcript_files(transcript: &Path) -> Vec<PathBuf> {
    let mut out = vec![transcript.to_path_buf()];
    if let (Some(dir), Some(stem)) = (transcript.parent(), transcript.file_stem()) {
        let sub = dir.join(stem).join("subagents");
        if let Ok(entries) = std::fs::read_dir(sub) {
            let mut subs: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("jsonl"))
                .collect();
            subs.sort();
            out.extend(subs);
        }
    }
    out
}

/// Atualiza o índice com o que cresceu nos arquivos da sessão.
pub fn update(index: &mut UsageIndex, transcript: &Path) -> Result<bool> {
    let files = transcript_files(transcript);
    // Arquivo encolheu (reescrito/rotacionado): não dá para descontar só a
    // parte dele, então reindexa tudo.
    let shrank = files.iter().any(|f| {
        let len = std::fs::metadata(f).map(|m| m.len()).unwrap_or(0);
        index
            .files
            .get(&f.to_string_lossy().to_string())
            .is_some_and(|c| len < c.offset)
    });
    if shrank || index.version != FORMAT_VERSION {
        *index = UsageIndex::default();
    }
    let main = transcript.to_string_lossy().to_string();
    let mut changed = shrank || index.transcript.as_deref() != Some(main.as_str());
    index.transcript = Some(main);
    for (i, f) in files.into_iter().enumerate() {
        let key = f.to_string_lossy().to_string();
        let cursor = index.files.entry(key).or_default();
        // O primeiro é o principal: só ele define o contexto da sessão.
        let insight = (i == 0).then_some(&mut index.insight);
        match read_file(&mut index.hours, cursor, &f, insight) {
            Ok(c) => changed |= c,
            // O principal ainda não existe no começo da sessão; subagente
            // pode sumir. Nada disso é erro para a TUI.
            Err(_) => continue,
        }
    }
    Ok(changed)
}

pub fn load(paths: &Paths, sid: &str) -> UsageIndex {
    std::fs::read(paths.usage_file(sid))
        .ok()
        .and_then(|b| serde_json::from_slice::<UsageIndex>(&b).ok())
        .filter(|i| i.version == FORMAT_VERSION)
        .unwrap_or_default()
}

pub fn save(paths: &Paths, sid: &str, index: &UsageIndex) -> Result<()> {
    write_atomic(&paths.usage_file(sid), &serde_json::to_vec(index)?)
}

pub fn remove(paths: &Paths, sid: &str) {
    let _ = std::fs::remove_file(paths.usage_file(sid));
}

/// Preço do modelo: vence o prefixo mais longo da tabela.
pub fn price_for<'a>(prices: &'a BTreeMap<String, Price>, model: &str) -> Option<&'a Price> {
    prices
        .iter()
        .filter(|(p, _)| model.starts_with(p.as_str()))
        .max_by_key(|(p, _)| p.len())
        .map(|(_, v)| v)
}

/// Custo equivalente em API (USD). `None` = modelo sem preço configurado.
pub fn cost(prices: &BTreeMap<String, Price>, model: &str, t: &Tokens) -> Option<f64> {
    let p = price_for(prices, model)?;
    let m = 1_000_000.0;
    Some(
        t.input as f64 * p.input / m
            + t.output as f64 * p.output / m
            + t.cache_write_5m as f64 * p.cache_write_5m / m
            + t.cache_write_1h as f64 * p.cache_write_1h / m
            + t.cache_read as f64 * p.cache_read / m,
    )
}

/// Janela de contexto do modelo: vence o prefixo mais longo (ADR-0012).
pub fn window_for(windows: &BTreeMap<String, u64>, model: &str) -> Option<u64> {
    windows
        .iter()
        .filter(|(p, _)| model.starts_with(p.as_str()))
        .max_by_key(|(p, _)| p.len())
        .map(|(_, v)| *v)
}

/// Modelo sem os sufixos internos (`+fast`, `+long`), para exibir.
pub fn display_model(model: &str) -> &str {
    model.split('+').next().unwrap_or(model)
}

// --- varredura em segundo plano -------------------------------------------

pub struct Job {
    pub session: String,
    pub transcript: PathBuf,
}

type Msg = (String, Hours, Insight);

/// Indexa transcripts numa thread e devolve os agregados por canal.
pub struct UsageScanner {
    tx: Sender<Msg>,
    rx: Receiver<Msg>,
    in_flight: usize,
    pub last: Option<std::time::Instant>,
    pub hours: std::collections::HashMap<String, Hours>,
    pub insight: std::collections::HashMap<String, Insight>,
    /// Sessão → transcript principal conhecido (do hook ou do índice salvo).
    pub transcripts: std::collections::HashMap<String, PathBuf>,
}

impl Default for UsageScanner {
    fn default() -> Self {
        let (tx, rx) = channel();
        Self {
            tx,
            rx,
            in_flight: 0,
            last: None,
            hours: Default::default(),
            insight: Default::default(),
            transcripts: Default::default(),
        }
    }
}

impl UsageScanner {
    pub fn busy(&self) -> bool {
        self.in_flight > 0
    }

    /// Carrega o que já está no disco, sem ler transcripts.
    pub fn preload(&mut self, paths: &Paths, sessions: &[String]) {
        for sid in sessions {
            let idx = load(paths, sid);
            if let Some(t) = &idx.transcript {
                self.transcripts.insert(sid.clone(), PathBuf::from(t));
            }
            self.insight.insert(sid.clone(), idx.insight);
            if !idx.hours.is_empty() {
                self.hours.insert(sid.clone(), idx.hours);
            }
        }
    }

    pub fn spawn(&mut self, paths: &Paths, jobs: Vec<Job>) {
        if jobs.is_empty() {
            return;
        }
        self.in_flight += 1;
        let tx = self.tx.clone();
        let paths = paths.clone();
        std::thread::spawn(move || {
            for j in jobs {
                let mut idx = load(&paths, &j.session);
                if let Ok(true) = update(&mut idx, &j.transcript) {
                    let _ = save(&paths, &j.session, &idx);
                    if tx.send((j.session, idx.hours, idx.insight)).is_err() {
                        return;
                    }
                }
            }
            // Marcador de fim do lote.
            let _ = tx.send((String::new(), Hours::new(), Insight::default()));
        });
    }

    /// Recolhe resultados prontos. Retorna se algo mudou.
    pub fn collect(&mut self) -> bool {
        let mut changed = false;
        while let Ok((sid, hours, insight)) = self.rx.try_recv() {
            if sid.is_empty() {
                self.in_flight = self.in_flight.saturating_sub(1);
                continue;
            }
            self.insight.insert(sid.clone(), insight);
            self.hours.insert(sid, hours);
            changed = true;
        }
        changed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    const FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/transcripts");

    fn total(h: &Hours) -> Tokens {
        let mut t = Tokens::default();
        for m in h.values() {
            for v in m.values() {
                t.add(v);
            }
        }
        t
    }

    fn tmp(name: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("ccdeck-usage-{}", crate::model::short_id()));
        std::fs::create_dir_all(&d).unwrap();
        d.join(name)
    }

    #[test]
    fn fixture_principal_ignora_synthetic() {
        let mut idx = UsageIndex::default();
        let p = Path::new(FIXTURES).join("principal-com-recusa.jsonl");
        assert!(update(&mut idx, &p).unwrap());
        let t = total(&idx.hours);
        // 6 linhas de assistente, 1 delas `<synthetic>`.
        assert_eq!(t.messages, 5);
        assert_eq!(t.input, 10);
        assert_eq!(t.output, 84 * 4 + 10);
        assert_eq!(t.cache_write_1h, 12134 + 4117 + 260 + 260 + 244);
        assert_eq!(t.cache_write_5m, 0);
        assert!(
            idx.hours
                .values()
                .all(|m| m.keys().all(|k| k == "claude-opus-5-5"))
        );
        // Ler de novo sem crescer não muda nada.
        assert!(!update(&mut idx, &p).unwrap());
    }

    #[test]
    fn fixture_subagente_conta_ultima_linha() {
        let mut idx = UsageIndex::default();
        let p = Path::new(FIXTURES).join("subagente-streaming.jsonl");
        update(&mut idx, &p).unwrap();
        let t = total(&idx.hours);
        // 7 linhas, 3 mensagens; valem os outputs finais 220, 167 e 5564.
        assert_eq!(t.messages, 3);
        assert_eq!(t.output, 220 + 167 + 5564);
    }

    #[test]
    fn linha_desconhecida_e_invalida_sao_ignoradas() {
        assert!(parse_line("não é json").is_none());
        assert!(parse_line(r#"{"type":"assistant","message":{"id":"m","model":"x"}}"#).is_none());
        assert!(parse_line(r#"{"type":"user","message":{"role":"assistant"}}"#).is_none());
        let c = parse_line(
            r#"{"type":"assistant","timestamp":"2026-10-23T14:05:00Z","message":{"id":"m1","model":"claude-haiku-5-5","usage":{"input_tokens":90000,"cache_read_input_tokens":20000,"output_tokens":5,"speed":"standard"}}}"#,
        )
        .unwrap();
        assert_eq!(c.hour, "2026-10-23T14");
        assert_eq!(c.model, "claude-haiku-5-5+long");
        let c = parse_line(
            r#"{"type":"assistant","timestamp":"2026-10-23T14:05:00Z","message":{"id":"m2","model":"claude-opus-5-5","usage":{"input_tokens":3,"cache_creation_input_tokens":40,"output_tokens":5,"speed":"fast"}}}"#,
        )
        .unwrap();
        assert_eq!(c.model, "claude-opus-5-5+fast");
        assert_eq!(c.tokens.cache_write_5m, 40, "sem detalhamento vale 5 min");
    }

    #[test]
    fn leitura_incremental_com_mensagem_dividida_entre_leituras() {
        let p = tmp("s.jsonl");
        let line = |id: &str, out: u64, h: &str| {
            format!(
                r#"{{"type":"assistant","timestamp":"2026-10-23T{h}:00:00Z","message":{{"id":"{id}","model":"claude-opus-5-5","usage":{{"input_tokens":2,"output_tokens":{out}}}}}}}"#
            )
        };
        let mut f = std::fs::File::create(&p).unwrap();
        writeln!(f, "{}", line("a", 5, "10")).unwrap();
        // Linha incompleta (ainda sendo escrita) não entra.
        write!(f, "{}", &line("a", 7, "10")[..30]).unwrap();
        f.flush().unwrap();
        let mut idx = UsageIndex::default();
        update(&mut idx, &p).unwrap();
        assert_eq!(total(&idx.hours).output, 5);

        // Completa a linha: mesma mensagem, a última vence.
        let rest = &line("a", 7, "10")[30..];
        writeln!(f, "{rest}").unwrap();
        writeln!(f, r#"{{"type":"user"}}"#).unwrap();
        writeln!(f, "{}", line("b", 11, "11")).unwrap();
        f.flush().unwrap();
        update(&mut idx, &p).unwrap();
        let t = total(&idx.hours);
        assert_eq!((t.messages, t.output), (2, 7 + 11));
        assert_eq!(idx.hours.len(), 2);

        // Arquivo reescrito menor: reindexa do zero.
        std::fs::write(&p, format!("{}\n", line("c", 3, "12"))).unwrap();
        update(&mut idx, &p).unwrap();
        assert_eq!(total(&idx.hours).output, 3);
        std::fs::remove_dir_all(p.parent().unwrap()).ok();
    }

    #[test]
    fn contexto_e_compactacao_so_do_principal() {
        let main = tmp("u-2.jsonl");
        let dir = main.parent().unwrap();
        let sub = dir.join("u-2").join("subagents");
        std::fs::create_dir_all(&sub).unwrap();
        let a = |id: &str, read: u64| {
            format!(
                r#"{{"type":"assistant","timestamp":"2026-10-23T10:00:00Z","message":{{"id":"{id}","model":"claude-opus-5-5","usage":{{"input_tokens":2,"cache_read_input_tokens":{read},"output_tokens":1,"speed":"fast"}}}}}}"#
            ) + "\n"
        };
        let compact = r#"{"type":"system","subtype":"compact_boundary","timestamp":"2026-10-23T10:05:00Z","compactMetadata":{"trigger":"auto","preTokens":968478,"postTokens":18395,"durationMs":1}}"#;
        std::fs::write(&main, a("m1", 900_000) + compact + "\n" + &a("m2", 20_000)).unwrap();
        std::fs::write(sub.join("agent-x.jsonl"), a("s1", 500_000)).unwrap();
        let mut idx = UsageIndex::default();
        update(&mut idx, &main).unwrap();
        let ctx = idx.insight.context.clone().unwrap();
        assert_eq!(ctx.prompt, 20_002, "última do principal, não do subagente");
        assert_eq!(ctx.model, "claude-opus-5-5");
        assert_eq!(idx.insight.compactions.len(), 1);
        assert_eq!(
            (
                idx.insight.compactions[0].pre,
                idx.insight.compactions[0].post
            ),
            (968_478, 18_395)
        );
        let w: BTreeMap<String, u64> = [
            ("claude-".to_string(), 1_000_000),
            ("claude-haiku-4-5".to_string(), 200_000),
        ]
        .into();
        assert_eq!(window_for(&w, "claude-opus-5-5"), Some(1_000_000));
        assert_eq!(window_for(&w, "claude-haiku-4-5"), Some(200_000));
        assert_eq!(window_for(&w, "gpt"), None);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn soma_subagentes() {
        let main = tmp("u-1.jsonl");
        let dir = main.parent().unwrap();
        let sub = dir.join("u-1").join("subagents");
        std::fs::create_dir_all(&sub).unwrap();
        let l = |id: &str| {
            format!(
                r#"{{"type":"assistant","timestamp":"2026-10-23T10:00:00Z","message":{{"id":"{id}","model":"claude-sonnet-5-5","usage":{{"output_tokens":10}}}}}}"#
            ) + "\n"
        };
        std::fs::write(&main, l("p")).unwrap();
        std::fs::write(sub.join("agent-x.jsonl"), l("s")).unwrap();
        std::fs::write(sub.join("notas.txt"), "x").unwrap();
        let mut idx = UsageIndex::default();
        update(&mut idx, &main).unwrap();
        assert_eq!(total(&idx.hours).output, 20);
        assert_eq!(idx.files.len(), 2);
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn custo_por_prefixo_mais_longo() {
        let prices = crate::model::default_prices();
        let t = Tokens {
            input: 1_000_000,
            output: 1_000_000,
            cache_write_5m: 1_000_000,
            cache_write_1h: 1_000_000,
            cache_read: 1_000_000,
            messages: 1,
        };
        // Opus 5.5: 4 + 20 + 5 + 8 + 0.20 (e não o preço do prefixo claude-opus-5).
        let c = cost(&prices, "claude-opus-5-5", &t).unwrap();
        assert!((c - 37.20).abs() < 1e-9, "{c}");
        let c = cost(&prices, "claude-opus-5-5+long", &t).unwrap();
        assert!((c - 37.20).abs() < 1e-9, "+long só muda o Haiku");
        let c = cost(&prices, "claude-haiku-5-5+long", &t).unwrap();
        assert!((c - (0.5 + 2.5 + 0.625 + 1.0 + 0.05)).abs() < 1e-9, "{c}");
        assert!(cost(&prices, "modelo-novo", &t).is_none());
        assert_eq!(
            display_model("claude-opus-5-5+fast+long"),
            "claude-opus-5-5"
        );
    }
}
