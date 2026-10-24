# Change: broadcast de prompt

## Why
A mesma resposta para várias sessões paradas esperando o usuário, sem um `m` por sessão (ADR-0016).

## What Changes
- `B` abre o broadcast com texto e lista de sessões elegíveis (vivas, em *Aguardando você*).
- Confirmação obrigatória com os destinos; reconferência do estado no envio; registro no `actions.log`.

## Impact
- Specs: `broadcast`
- Código: novo `broadcast.rs`, `app.rs`, `ui.rs`
- ADR: 0016
