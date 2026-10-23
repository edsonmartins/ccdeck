# Change: alterações por sessão e aviso de colisão

## Why
Revisar exige saber o que cada sessão mudou, e conflitos entre sessões do mesmo repositório só aparecem no merge (ADR-0009).

## What Changes
- Bloco **Alterações** no painel: arquivos com +/−, incluindo não rastreados.
- Cálculo em segundo plano para todas as sessões.
- `⚠` na sidebar e lista de arquivos em colisão no painel.
- `D` abre o diff completo com `diff_command` configurável.

## Impact
- Specs: `change-review`
- Código: `git.rs`, novo `changes.rs`, `app.rs`, `ui.rs`, `main.rs`, `model.rs` (config)
- ADR: 0009
