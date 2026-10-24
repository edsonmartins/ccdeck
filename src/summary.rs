//! Agregados da aba Resumo (ADR-0010). Funções puras: recebem os dados já
//! carregados e devolvem o que a UI desenha.

use std::collections::{BTreeMap, HashMap};

use chrono::{DateTime, Duration, Local, NaiveDateTime, TimeZone, Timelike, Utc};

use crate::events::{self, Event};
use crate::git::FileChange;
use crate::model::{Price, Session, Status};
use crate::usage::{self, Hours, Tokens};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Period {
    Today,
    Week,
    Month,
}

impl Period {
    pub fn label(self) -> &'static str {
        match self {
            Period::Today => "hoje",
            Period::Week => "7 dias",
            Period::Month => "30 dias",
        }
    }

    pub fn next(self) -> Self {
        match self {
            Period::Today => Period::Week,
            Period::Week => Period::Month,
            Period::Month => Period::Today,
        }
    }

    /// Início do período no horário local (hoje = desde a meia-noite).
    pub fn start(self, now: DateTime<Local>) -> DateTime<Utc> {
        let midnight = now
            .date_naive()
            .and_hms_opt(0, 0, 0)
            .and_then(|n| Local.from_local_datetime(&n).earliest())
            .unwrap_or(now);
        let days = match self {
            Period::Today => 0,
            Period::Week => 6,
            Period::Month => 29,
        };
        (midnight - Duration::days(days)).with_timezone(&Utc)
    }

    /// Hoje: barras por hora; 7 e 30 dias: por dia.
    pub fn hourly(self) -> bool {
        self == Period::Today
    }
}

/// Converte a chave do índice (`AAAA-MM-DDTHH`, UTC) em horário.
pub fn parse_hour(key: &str) -> Option<DateTime<Utc>> {
    NaiveDateTime::parse_from_str(&format!("{key}:00:00"), "%Y-%m-%dT%H:%M:%S")
        .ok()
        .map(|n| n.and_utc())
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Cost {
    pub usd: f64,
    /// Parte do uso veio de modelo sem preço: o valor é um piso.
    pub unknown: bool,
}

impl Cost {
    fn add(&mut self, c: Option<f64>) {
        match c {
            Some(v) => self.usd += v,
            None => self.unknown = true,
        }
    }
}

#[derive(Debug, Clone)]
pub struct Row {
    pub session: String,
    pub name: String,
    pub project: String,
    pub status: Status,
    pub tokens: Tokens,
    pub cost: Cost,
    pub waiting: Duration,
}

#[derive(Debug, Clone)]
pub struct Bucket {
    pub start: DateTime<Local>,
    pub tokens: u64,
    pub cost: f64,
    pub messages: u64,
}

#[derive(Debug, Clone)]
pub struct Activity {
    pub at: DateTime<Utc>,
    pub session: String,
    pub event: String,
    pub status: Status,
}

#[derive(Debug, Clone)]
pub struct Summary {
    pub period: Period,
    pub tokens: Tokens,
    pub cost: Cost,
    pub waiting: Duration,
    pub buckets: Vec<Bucket>,
    pub rows: Vec<Row>,
    pub activity: Vec<Activity>,
    /// (arquivo, sessão, +, −)
    pub files: Vec<(String, String, u32, u32)>,
    pub unpriced: Vec<String>,
}

impl Summary {
    /// Leitura de cache sobre tudo que entrou no modelo.
    pub fn cache_rate(&self) -> Option<f64> {
        let p = self.tokens.prompt();
        (p > 0).then(|| self.tokens.cache_read as f64 / p as f64)
    }
}

pub struct Input<'a> {
    pub sessions: Vec<(&'a Session, &'a str, Status, bool)>,
    pub hours: &'a HashMap<String, Hours>,
    pub events: &'a HashMap<String, Vec<Event>>,
    pub changes: &'a HashMap<String, Vec<FileChange>>,
    pub prices: &'a BTreeMap<String, Price>,
}

/// Agrega o período. `sessions` = (sessão, nome do projeto, estado, viva).
pub fn compute(input: &Input, period: Period, now: DateTime<Utc>) -> Summary {
    let local_now = now.with_timezone(&Local);
    let from = period.start(local_now);
    let bucket_of = |t: DateTime<Utc>| -> DateTime<Local> {
        let l = t.with_timezone(&Local);
        let naive = if period.hourly() {
            l.date_naive().and_hms_opt(l.hour(), 0, 0)
        } else {
            l.date_naive().and_hms_opt(0, 0, 0)
        };
        naive
            .and_then(|n| Local.from_local_datetime(&n).earliest())
            .unwrap_or(l)
    };
    // Buckets vazios do período inteiro, para o gráfico não pular horas.
    let mut buckets: BTreeMap<DateTime<Local>, Bucket> = BTreeMap::new();
    let step = if period.hourly() {
        Duration::hours(1)
    } else {
        Duration::days(1)
    };
    let mut t = from.with_timezone(&Local);
    while t <= local_now {
        let b = bucket_of(t.with_timezone(&Utc));
        buckets.entry(b).or_insert(Bucket {
            start: b,
            tokens: 0,
            cost: 0.0,
            messages: 0,
        });
        t += step;
    }

    let mut s = Summary {
        period,
        tokens: Tokens::default(),
        cost: Cost::default(),
        waiting: Duration::zero(),
        buckets: Vec::new(),
        rows: Vec::new(),
        activity: Vec::new(),
        files: Vec::new(),
        unpriced: Vec::new(),
    };
    let mut unpriced = std::collections::BTreeSet::new();
    for (sess, project, status, alive) in &input.sessions {
        let mut row = Row {
            session: sess.id.clone(),
            name: sess.name.clone(),
            project: project.to_string(),
            status: *status,
            tokens: Tokens::default(),
            cost: Cost::default(),
            waiting: Duration::zero(),
        };
        if let Some(h) = input.hours.get(&sess.id) {
            for (key, models) in h {
                let Some(at) = parse_hour(key) else { continue };
                // A hora inteira entra se ela termina depois do início.
                if at + Duration::hours(1) <= from || at > now {
                    continue;
                }
                for (model, tok) in models {
                    let c = usage::cost(input.prices, model, tok);
                    if c.is_none() {
                        unpriced.insert(usage::display_model(model).to_string());
                    }
                    row.tokens.add(tok);
                    row.cost.add(c);
                    let b = buckets.entry(bucket_of(at.max(from))).or_insert(Bucket {
                        start: bucket_of(at.max(from)),
                        tokens: 0,
                        cost: 0.0,
                        messages: 0,
                    });
                    b.tokens += tok.prompt() + tok.output;
                    b.cost += c.unwrap_or(0.0);
                    b.messages += tok.messages;
                }
            }
        }
        if let Some(evs) = input.events.get(&sess.id) {
            let end = if *alive {
                now
            } else {
                evs.last().map(|e| e.at).unwrap_or(now)
            };
            row.waiting = events::waiting(&events::time_in_states(evs, from, now, end));
            s.activity
                .extend(evs.iter().filter(|e| e.at >= from).map(|e| Activity {
                    at: e.at,
                    session: sess.name.clone(),
                    event: e.event.clone(),
                    status: e.status,
                }));
        }
        for c in input
            .changes
            .get(&sess.id)
            .map(Vec::as_slice)
            .unwrap_or_default()
        {
            s.files.push((
                c.path.clone(),
                sess.name.clone(),
                c.added.unwrap_or(0),
                c.deleted.unwrap_or(0),
            ));
        }
        s.tokens.add(&row.tokens);
        s.cost.usd += row.cost.usd;
        s.cost.unknown |= row.cost.unknown;
        s.waiting += row.waiting;
        s.rows.push(row);
    }
    s.rows
        .sort_by_key(|r| std::cmp::Reverse(r.tokens.prompt() + r.tokens.output));
    s.activity.sort_by_key(|a| std::cmp::Reverse(a.at));
    s.activity.truncate(50);
    s.files.sort_by_key(|f| std::cmp::Reverse(f.2 + f.3));
    s.buckets = buckets.into_values().collect();
    s.unpriced = unpriced.into_iter().collect();
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sess(id: &str) -> Session {
        Session {
            id: id.into(),
            project_id: "p".into(),
            name: format!("sessao-{id}"),
            cwd: "/tmp".into(),
            branch: None,
            worktree: false,
            claude_session_id: uuid::Uuid::new_v4(),
            permission_mode: "default".into(),
            notes: String::new(),
            created_at: Utc::now(),
            started_once: true,
            base: None,
            port_slot: None,
            ports: vec![],
        }
    }

    fn tok(input: u64, output: u64) -> Tokens {
        Tokens {
            input,
            output,
            messages: 1,
            ..Tokens::default()
        }
    }

    #[test]
    fn periodo_custo_espera_e_modelo_sem_preco() {
        let now = Utc::now();
        let hour = |h: i64| (now - Duration::hours(h)).format("%Y-%m-%dT%H").to_string();
        let a = sess("a");
        let mut hours: HashMap<String, Hours> = HashMap::new();
        let mut h: Hours = BTreeMap::new();
        h.entry(hour(0))
            .or_default()
            .insert("claude-opus-5-5".into(), tok(1_000_000, 0));
        h.entry(hour(0))
            .or_default()
            .insert("modelo-x".into(), tok(5, 5));
        // 10 dias atrás: fora de "7 dias", dentro de "30 dias".
        h.entry(hour(24 * 10))
            .or_default()
            .insert("claude-opus-5-5".into(), tok(0, 1_000_000));
        hours.insert("a".into(), h);
        let mut evs = HashMap::new();
        evs.insert(
            "a".to_string(),
            vec![
                Event {
                    at: now - Duration::minutes(30),
                    status: Status::Approval,
                    event: "PermissionRequest · Bash".into(),
                },
                Event {
                    at: now - Duration::minutes(23),
                    status: Status::Working,
                    event: "PostToolUse · Bash".into(),
                },
                Event {
                    at: now - Duration::minutes(10),
                    status: Status::Yours,
                    event: "Stop".into(),
                },
            ],
        );
        let changes = HashMap::new();
        let prices = crate::model::default_prices();
        let input = Input {
            sessions: vec![(&a, "proj", Status::Yours, true)],
            hours: &hours,
            events: &evs,
            changes: &changes,
            prices: &prices,
        };
        let w = compute(&input, Period::Week, now);
        assert_eq!(w.tokens.input, 1_000_005);
        assert!((w.cost.usd - 4.0).abs() < 1e-9, "{}", w.cost.usd);
        assert!(w.cost.unknown);
        assert_eq!(w.unpriced, ["modelo-x"]);
        // 7 min em aprovação + 10 min aguardando você até agora.
        assert_eq!(w.waiting, Duration::minutes(17));
        assert_eq!(w.activity.len(), 3);
        assert_eq!(w.activity[0].event, "Stop", "mais recente primeiro");
        assert_eq!(w.buckets.len(), 7);

        let m = compute(&input, Period::Month, now);
        assert!((m.cost.usd - 24.0).abs() < 1e-9, "{}", m.cost.usd);
        assert_eq!(m.buckets.len(), 30);
        assert_eq!(m.buckets.iter().map(|b| b.messages).sum::<u64>(), 3);

        let t = compute(&input, Period::Today, now);
        assert!(t.buckets.len() <= 24);
    }

    #[test]
    fn chave_de_hora() {
        assert_eq!(
            parse_hour("2026-10-23T14").unwrap().to_rfc3339(),
            "2026-10-23T14:00:00+00:00"
        );
        assert!(parse_hour("lixo").is_none());
    }
}
