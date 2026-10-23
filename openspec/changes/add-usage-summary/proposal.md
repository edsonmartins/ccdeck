# Change: resumo de uso

## Why
Com várias sessões em paralelo falta visão de consumo (tokens, custo equivalente, cache) e de gargalo (tempo que as sessões ficam esperando o usuário). Ver ADR-0010.

## What Changes
- Indexação incremental do transcript de cada sessão (tokens por hora e por modelo).
- Histórico de eventos de hook por sessão.
- Tabela de preços configurável e custo "equivalente em API".
- Aba **Resumo** com blocos, gráficos, tabela por sessão, atividade recente e arquivos modificados; filtro de período e projeto.
- Linha compacta de uso no painel de cada sessão.

## Impact
- Specs: `usage-summary`
- Código: novo `usage.rs`, `hook.rs` (histórico), `changes.rs` (agenda a indexação), `model.rs` (config `prices`, `cost_display`), `app.rs` (`View::Summary`), `ui.rs` (aba e gráficos), `main.rs` (`doctor`)
- ADR: 0010 (Proposto até a aprovação das telas)
