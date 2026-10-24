//! Broadcast de prompt (ADR-0016): quem pode receber e quem vem marcado.

use crate::model::Status;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Target {
    pub session: String,
    pub checked: bool,
}

/// Candidata: (id, projeto, estado, viva).
pub type Candidate<'a> = (&'a str, &'a str, Status, bool);

/// Elegíveis = vivas em *Aguardando você*. Marcadas = as do projeto atual.
pub fn targets(candidates: &[Candidate], current_project: Option<&str>) -> Vec<Target> {
    candidates
        .iter()
        .filter(|(_, _, st, alive)| *alive && *st == Status::Yours)
        .map(|(id, proj, _, _)| Target {
            session: id.to_string(),
            checked: current_project.is_none_or(|p| p == *proj),
        })
        .collect()
}

/// Reconferência no envio: ainda viva e em *Aguardando você*.
pub fn still_eligible(status: Option<Status>, alive: bool) -> bool {
    alive && status == Some(Status::Yours)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn elegiveis_e_marcacao() {
        let c = [
            ("a", "p1", Status::Yours, true),
            ("b", "p2", Status::Yours, true),
            ("c", "p1", Status::Working, true),
            ("d", "p1", Status::Approval, true),
            ("e", "p1", Status::Yours, false),
        ];
        let t = targets(&c, Some("p1"));
        let v: Vec<(&str, bool)> = t.iter().map(|t| (t.session.as_str(), t.checked)).collect();
        assert_eq!(v, [("a", true), ("b", false)]);
        assert!(targets(&c, None).iter().all(|t| t.checked));
        assert!(still_eligible(Some(Status::Yours), true));
        assert!(!still_eligible(Some(Status::Working), true));
        assert!(!still_eligible(Some(Status::Yours), false));
        assert!(!still_eligible(None, true));
    }
}
