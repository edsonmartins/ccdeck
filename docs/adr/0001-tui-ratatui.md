# ADR-0001 — Interface TUI com ratatui

- Status: Aceito
- Data: 2026-10-23

## Contexto
O uso hoje já é todo no terminal. Precisamos de uma interface leve que rode local ou via SSH, em macOS e Linux.

## Decisão
TUI em Rust com `ratatui` 0.30 e backend `crossterm`. Loop de eventos síncrono com tick de 250 ms; sem runtime async na v0.1.

## Alternativas
- **Tauri + xterm.js**: visual mais rico, mas exige web view, não funciona via SSH e duplica o emulador de terminal.
- **egui/iced**: renderizar terminal com fidelidade é trabalho grande.

## Consequências
- Funciona por SSH e dentro do tmux.
- Não renderizamos o Claude Code dentro do app: o "entrar na sessão" é um attach do tmux (ADR-0002).
