# Change: entrega (commit, push, PR) e status do CI

## Why
Fechar uma sessão sem sair do ccdeck e acompanhar o CI do PR dela (ADR-0017).

## What Changes
- `E`: modal de entrega com commit, push e PR, cada um com confirmação; execução em segundo plano.
- CI do PR aberto ao lado do branch no painel; consulta por projeto a cada 60 s.
- `config.json`: `gh_command`.

## Impact
- Specs: `delivery`
- Código: novo `delivery.rs`, `git.rs`, `model.rs`, `app.rs`, `ui.rs`; `scripts/fake-gh.sh` para testes
- ADR: 0017
