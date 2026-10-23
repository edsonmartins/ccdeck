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
    Name,
    Base,
    Worktree,
    Mode,
    Prompt,
}

const FIELDS: [Field; 5] = [
    Field::Name,
    Field::Base,
    Field::Worktree,
    Field::Mode,
    Field::Prompt,
];

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
}

pub enum FormOutcome {
    Continue,
    Submit,
    Cancel,
}

impl SessionForm {
    pub fn focused(&self) -> Field {
        FIELDS[self.focus]
    }

    pub fn mode(&self) -> &'static str {
        PERMISSION_MODES[self.mode]
    }

    pub fn handle(&mut self, key: KeyEvent) -> FormOutcome {
        match key.code {
            KeyCode::Esc => return FormOutcome::Cancel,
            KeyCode::Enter => return FormOutcome::Submit,
            KeyCode::Tab | KeyCode::Down => self.focus = (self.focus + 1) % FIELDS.len(),
            KeyCode::BackTab | KeyCode::Up => {
                self.focus = (self.focus + FIELDS.len() - 1) % FIELDS.len()
            }
            _ => match self.focused() {
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
