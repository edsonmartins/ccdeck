# Change: janelas auxiliares por sessão

## Why
Shell, servidor e logs junto de cada sessão, no worktree e nas portas dela, sem outro terminal (ADR-0015).

## What Changes
- `.ccdeck.json`: `windows` (nome → comando).
- `w` lista as janelas da sessão; `Enter` cria e entra; `K` fecha (com confirmação).

## Impact
- Specs: `aux-windows`
- Código: `project.rs`, `tmux.rs`, `app.rs`, `ui.rs`, `main.rs`
- ADR: 0015
