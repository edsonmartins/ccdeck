//! Campos de texto e formulários usados nos modais.

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

use crate::model::PERMISSION_MODES;

#[derive(Debug, Clone, Default)]
pub struct TextInput {
    pub value: String,
    /// Posição do cursor em caracteres.
    pub cursor: usize,
    pub multiline: bool,
}

impl TextInput {
    pub fn new(value: impl Into<String>) -> Self {
        let value = value.into();
        let cursor = value.chars().count();
        Self {
            value,
            cursor,
            multiline: false,
        }
    }

    pub fn multiline(value: impl Into<String>) -> Self {
        Self {
            multiline: true,
            ..Self::new(value)
        }
    }

    fn byte_idx(&self, char_idx: usize) -> usize {
        self.value
            .char_indices()
            .nth(char_idx)
            .map(|(i, _)| i)
            .unwrap_or(self.value.len())
    }

    /// Retorna true se a tecla foi consumida.
    pub fn handle(&mut self, key: KeyEvent) -> bool {
        let len = self.value.chars().count();
        match key.code {
            KeyCode::Char(c) if !key.modifiers.contains(KeyModifiers::CONTROL) => {
                let i = self.byte_idx(self.cursor);
                self.value.insert(i, c);
                self.cursor += 1;
            }
            KeyCode::Enter if self.multiline => {
                let i = self.byte_idx(self.cursor);
                self.value.insert(i, '\n');
                self.cursor += 1;
            }
            KeyCode::Backspace if self.cursor > 0 => {
                let i = self.byte_idx(self.cursor - 1);
                self.value.remove(i);
                self.cursor -= 1;
            }
            KeyCode::Delete if self.cursor < len => {
                let i = self.byte_idx(self.cursor);
                self.value.remove(i);
            }
            KeyCode::Left => self.cursor = self.cursor.saturating_sub(1),
            KeyCode::Right => self.cursor = (self.cursor + 1).min(len),
            KeyCode::Home => self.cursor = 0,
            KeyCode::End => self.cursor = len,
            KeyCode::Char('u') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                self.value.clear();
                self.cursor = 0;
            }
            _ => return false,
        }
        true
    }

    /// Texto com um marcador de cursor, para desenhar.
    pub fn with_cursor(&self, focused: bool) -> String {
        if !focused {
            return self.value.clone();
        }
        let i = self.byte_idx(self.cursor);
        format!("{}▏{}", &self.value[..i], &self.value[i..])
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Field {
    /// Modelo de sessão (ADR-0014); só aparece quando há opções.
    Starter,
    Name,
    Base,
    Worktree,
    Mode,
    Prompt,
}

const FIELDS: [Field; 6] = [
    Field::Starter,
    Field::Name,
    Field::Base,
    Field::Worktree,
    Field::Mode,
    Field::Prompt,
];

/// Opção do campo Modelo: template do projeto ou change do OpenSpec.
#[derive(Debug, Clone, PartialEq)]
pub struct Starter {
    pub label: String,
    /// Nome sugerido para a sessão.
    pub name: String,
    pub prompt: String,
    /// Índice em `PERMISSION_MODES`.
    pub mode: Option<usize>,
    pub warning: Option<String>,
}

#[derive(Debug, Clone)]
pub struct SessionForm {
    /// Resumo do preparo do worktree vindo do `.ccdeck.json` (ADR-0008).
    pub prep: Vec<String>,
    pub project_id: String,
    pub project_name: String,
    pub is_repo: bool,
    pub name: TextInput,
    pub base: TextInput,
    pub worktree: bool,
    pub mode: usize,
    pub prompt: TextInput,
    pub focus: usize,
    pub starters: Vec<Starter>,
    /// 0 = nenhum; `i` = `starters[i - 1]`.
    pub starter: usize,
}

pub enum FormOutcome {
    Continue,
    Submit,
    Cancel,
}

impl SessionForm {
    /// Campos visíveis, na ordem do Tab.
    pub fn fields(&self) -> Vec<Field> {
        FIELDS
            .into_iter()
            .filter(|f| *f != Field::Starter || !self.starters.is_empty())
            .collect()
    }

    pub fn focused(&self) -> Field {
        let f = self.fields();
        f[self.focus.min(f.len() - 1)]
    }

    pub fn current_starter(&self) -> Option<&Starter> {
        self.starter
            .checked_sub(1)
            .and_then(|i| self.starters.get(i))
    }

    /// Troca o modelo. Só sobrescreve nome e prompt que o usuário não editou
    /// (vazios ou iguais aos do modelo anterior).
    fn select_starter(&mut self, next: usize) {
        let prev = self.current_starter().cloned();
        self.starter = next;
        let new = self.current_starter().cloned();
        let untouched = |v: &str, old: Option<&str>| v.trim().is_empty() || Some(v) == old;
        if untouched(&self.name.value, prev.as_ref().map(|p| p.name.as_str())) {
            self.name = TextInput::new(new.as_ref().map(|n| n.name.clone()).unwrap_or_default());
        }
        if untouched(&self.prompt.value, prev.as_ref().map(|p| p.prompt.as_str())) {
            self.prompt =
                TextInput::new(new.as_ref().map(|n| n.prompt.clone()).unwrap_or_default());
        }
        match new.as_ref().and_then(|n| n.mode) {
            Some(m) => self.mode = m,
            // O modo veio do modelo anterior e o usuário não mexeu: volta ao padrão.
            None if prev.as_ref().and_then(|p| p.mode) == Some(self.mode) => self.mode = 0,
            None => {}
        }
    }

    pub fn mode(&self) -> &'static str {
        PERMISSION_MODES[self.mode]
    }

    pub fn handle(&mut self, key: KeyEvent) -> FormOutcome {
        match key.code {
            KeyCode::Esc => return FormOutcome::Cancel,
            KeyCode::Enter => return FormOutcome::Submit,
            KeyCode::Tab | KeyCode::Down => self.focus = (self.focus + 1) % self.fields().len(),
            KeyCode::BackTab | KeyCode::Up => {
                let n = self.fields().len();
                self.focus = (self.focus + n - 1) % n
            }
            _ => match self.focused() {
                Field::Starter => {
                    let n = self.starters.len() + 1;
                    match key.code {
                        KeyCode::Char(' ') | KeyCode::Right => {
                            self.select_starter((self.starter + 1) % n)
                        }
                        KeyCode::Left => self.select_starter((self.starter + n - 1) % n),
                        _ => {}
                    }
                }
                Field::Name => {
                    self.name.handle(key);
                }
                Field::Base => {
                    self.base.handle(key);
                }
                Field::Prompt => {
                    self.prompt.handle(key);
                }
                Field::Worktree => {
                    if matches!(
                        key.code,
                        KeyCode::Char(' ') | KeyCode::Left | KeyCode::Right
                    ) && self.is_repo
                    {
                        self.worktree = !self.worktree;
                    }
                }
                Field::Mode => match key.code {
                    KeyCode::Char(' ') | KeyCode::Right => {
                        self.mode = (self.mode + 1) % PERMISSION_MODES.len()
                    }
                    KeyCode::Left => {
                        self.mode =
                            (self.mode + PERMISSION_MODES.len() - 1) % PERMISSION_MODES.len()
                    }
                    _ => {}
                },
            },
        }
        FormOutcome::Continue
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn k(c: KeyCode) -> KeyEvent {
        KeyEvent::new(c, KeyModifiers::NONE)
    }

    fn form(starters: Vec<Starter>) -> SessionForm {
        SessionForm {
            prep: vec![],
            project_id: "p".into(),
            project_name: "p".into(),
            is_repo: true,
            name: TextInput::new(""),
            base: TextInput::new("main"),
            worktree: true,
            mode: 0,
            prompt: TextInput::new(""),
            focus: 0,
            starters,
            starter: 0,
        }
    }

    #[test]
    fn modelo_preenche_sem_apagar_o_que_o_usuario_digitou() {
        let st = |name: &str, mode| Starter {
            label: name.into(),
            name: name.into(),
            prompt: format!("prompt de {name}"),
            mode,
            warning: None,
        };
        let mut f = form(vec![st("add-x", None), st("revisar", Some(2))]);
        assert_eq!(f.focused(), Field::Starter);
        f.handle(k(KeyCode::Right));
        assert_eq!(
            (f.name.value.as_str(), f.prompt.value.as_str()),
            ("add-x", "prompt de add-x")
        );
        f.handle(k(KeyCode::Right));
        assert_eq!(f.name.value, "revisar");
        assert_eq!(f.mode, 2, "template com modo");
        // Do template com modo para o change sem modo: volta ao padrão.
        f.handle(k(KeyCode::Left));
        assert_eq!((f.name.value.as_str(), f.mode), ("add-x", 0));
        f.handle(k(KeyCode::Right));
        assert_eq!(f.mode, 2);
        // O usuário edita o nome; trocar de modelo mantém o nome dele.
        f.name = TextInput::new("meu-nome");
        f.handle(k(KeyCode::Right));
        assert_eq!(f.starter, 0);
        assert_eq!(f.name.value, "meu-nome");
        assert_eq!(f.prompt.value, "", "prompt não editado volta a vazio");
        // Sem modelos o campo some e o foco começa no nome.
        let f = form(vec![]);
        assert_eq!(f.focused(), Field::Name);
        assert_eq!(f.fields().len(), 5);
    }

    #[test]
    fn edita_com_acentos() {
        let mut t = TextInput::new("migração");
        t.handle(k(KeyCode::Backspace));
        t.handle(k(KeyCode::Home));
        t.handle(k(KeyCode::Char('x')));
        assert_eq!(t.value, "xmigraçã");
        assert_eq!(t.with_cursor(true), "x▏migraçã");
    }
}
