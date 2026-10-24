//! Linha do tempo da sessão (ADR-0012): eventos de hook, ações do ccdeck e
//! compactações, mais recentes primeiro.

use chrono::{DateTime, Duration, Utc};

use crate::events::Event;
use crate::model::Status;
use crate::paths::Paths;
use crate::usage::Compaction;

#[derive(Debug, Clone, PartialEq)]
pub enum Kind {
    /// Evento de hook; a duração é quanto a sessão ficou nesse estado.
    State(Status, Option<Duration>),
    /// Ação do ccdeck registrada no `actions.log` (aprovar, recusar, responder).
    Action,
    Compaction,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Entry {
    pub at: DateTime<Utc>,
    pub kind: Kind,
    pub text: String,
}

/// Linha do `actions.log`: `horário \t sid \t nome \t ação \t detalhe`.
pub fn load_actions(paths: &Paths, sid: &str) -> Vec<(DateTime<Utc>, String)> {
    let Ok(txt) = std::fs::read_to_string(paths.state_dir.join("actions.log")) else {
        return Vec::new();
    };
    txt.lines()
        .filter_map(|l| {
            let mut p = l.splitn(5, '\t');
            let at: DateTime<Utc> = p.next()?.parse().ok()?;
            (p.next()? == sid).then_some(())?;
            let _name = p.next()?;
            let action = p.next()?;
            let detail = p.next().unwrap_or("");
            Some((at, format!("{action}  {detail}")))
        })
        .collect()
}

/// Monta a linha do tempo. `end` = até quando vale o último estado (agora se
/// a sessão está viva).
pub fn build(
    events: &[Event],
    actions: &[(DateTime<Utc>, String)],
    compactions: &[Compaction],
    end: DateTime<Utc>,
) -> Vec<Entry> {
    let mut out: Vec<Entry> = events
        .iter()
        .enumerate()
        .map(|(i, e)| {
            let until = events.get(i + 1).map(|n| n.at).or(Some(end));
            let dur = until.map(|u| u - e.at).filter(|d| *d >= Duration::zero());
            Entry {
                at: e.at,
                kind: Kind::State(e.status, dur),
                text: e.event.clone(),
            }
        })
        .collect();
    out.extend(actions.iter().map(|(at, text)| Entry {
        at: *at,
        kind: Kind::Action,
        text: text.clone(),
    }));
    out.extend(compactions.iter().map(|c| Entry {
        at: c.at,
        kind: Kind::Compaction,
        text: format!("compactação {}  {} → {} tokens", c.trigger, c.pre, c.post),
    }));
    // Mais recente primeiro; no empate, ação antes do evento que ela causou.
    out.sort_by(|a, b| {
        b.at.cmp(&a.at)
            .then_with(|| matches!(b.kind, Kind::Action).cmp(&matches!(a.kind, Kind::Action)))
    });
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(s: i64) -> DateTime<Utc> {
        "2026-10-23T10:00:00Z".parse::<DateTime<Utc>>().unwrap() + Duration::seconds(s)
    }

    #[test]
    fn junta_eventos_acoes_e_compactacoes() {
        let evs = [
            Event {
                at: at(0),
                status: Status::Approval,
                event: "PermissionRequest · Bash".into(),
            },
            Event {
                at: at(82),
                status: Status::Yours,
                event: "ccdeck · recusado".into(),
            },
        ];
        let acts = [(at(82), "recusar  Bash: mkdir x".to_string())];
        let comps = [Compaction {
            at: at(30),
            trigger: "auto".into(),
            pre: 968_478,
            post: 18_395,
        }];
        let t = build(&evs, &acts, &comps, at(100));
        let kinds: Vec<&str> = t
            .iter()
            .map(|e| match e.kind {
                Kind::State(..) => "estado",
                Kind::Action => "ação",
                Kind::Compaction => "compactação",
            })
            .collect();
        assert_eq!(kinds, ["ação", "estado", "compactação", "estado"]);
        assert_eq!(
            t[3].kind,
            Kind::State(Status::Approval, Some(Duration::seconds(82)))
        );
        assert_eq!(
            t[1].kind,
            Kind::State(Status::Yours, Some(Duration::seconds(18)))
        );
    }

    #[test]
    fn le_actions_log_da_sessao() {
        let dir = std::env::temp_dir().join(format!("ccdeck-tl-{}", crate::model::short_id()));
        let paths = Paths::rooted(&dir);
        std::fs::create_dir_all(&paths.state_dir).unwrap();
        std::fs::write(
            paths.state_dir.join("actions.log"),
            "2026-10-23T23:54:36+00:00\ts1\taprovar\taprovar\tBash: mkdir pasta-y\n\
             2026-10-23T23:55:00+00:00\ts2\toutra\trecusar\tBash: rm\n\
             lixo\n",
        )
        .unwrap();
        let a = load_actions(&paths, "s1");
        assert_eq!(a.len(), 1);
        assert_eq!(a[0].1, "aprovar  Bash: mkdir pasta-y");
        std::fs::remove_dir_all(dir).ok();
    }
}
