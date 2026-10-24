# Change: estado depois de recusa ou interrupção

## Why
No Claude Code real, recusar um pedido ou interromper um turno não dispara hook; a sessão fica presa em *Aguardando aprovação* ou *Trabalhando* e o `m` fica bloqueado (`docs/validacao-v0.2.md` §5, ADR-0011).

## What Changes
- Depois de `x` bem-sucedido, o ccdeck grava *Aguardando você* (`ccdeck · recusado`) se o status não mudou no meio.
- Painel avisa "sem eventos há N min" em *Trabalhando* parado há 5 min ou mais.
- Modo de permissão `default` aparece na UI como "padrão das suas settings" (não passa `--permission-mode`).

## Impact
- Specs: `session-state`
- Código: `hook.rs` (registro local), `app.rs` (`answer_permission`), `ui.rs` (aviso e rótulo)
- ADR: 0011 (altera a invariante 2)
