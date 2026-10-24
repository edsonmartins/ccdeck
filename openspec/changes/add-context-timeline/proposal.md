# Change: contexto usado e linha do tempo por sessão

## Why
Sessões longas compactam sem aviso e é difícil saber o que aconteceu numa sessão enquanto o usuário não olhava (ADR-0012).

## What Changes
- Índice de uso v2: contexto da última mensagem do transcript principal, compactações (`compact_boundary`).
- `config.json: context_windows` por prefixo de modelo (padrão 1M).
- Painel: linha de contexto com % e compactações; destaque acima de 85%.
- `L` abre a linha do tempo da sessão (eventos + ações do ccdeck + compactações).

## Impact
- Specs: `session-insight`
- Código: `usage.rs`, `model.rs`, `app.rs`, `ui.rs`, novo `timeline.rs`
- ADR: 0012
