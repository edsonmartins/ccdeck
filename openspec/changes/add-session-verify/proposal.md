# Change: verificação por sessão

## Why
Rodar os testes no worktree de uma sessão sem entrar nela e sem parar o Claude Code (ADR-0013).

## What Changes
- `.ccdeck.json`: campo `verify`.
- `t` roda a verificação numa janela tmux `verify`; `T` abre o log.
- Painel mostra o estado da verificação; aviso e notificação ao terminar.

## Impact
- Specs: `session-verify`
- Código: `project.rs`, `app.rs`, `ui.rs`, `tmux.rs`
- ADR: 0013
