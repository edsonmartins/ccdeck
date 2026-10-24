# ADR-0012 — Contexto usado e linha do tempo por sessão

- Status: Aceito
- Data: 2026-10-23
- Complementa: ADR-0010

## Contexto
Com sessões longas, a pergunta é "quanto falta para compactar?" e "o que aconteceu nessa sessão enquanto eu não olhava?". O custo por sessão já existe (ADR-0010).

## Validação (transcripts desta máquina, Claude Code 2.1.310)
- [confirmado] O maior prompt visto por modelo foi ~998 mil tokens (Opus 5), ~967 mil (Opus 5.5, Sonnet 5.5, Fable 5.1): a janela usada pelo Claude Code é de 1M.
- [confirmado] A compactação grava uma linha `type: "system"`, `subtype: "compact_boundary"`, com `compactMetadata.{trigger, preTokens, postTokens, durationMs}`. As automáticas dispararam com `preTokens` entre 967 e 969 mil e caíram para 10–18 mil.
- [inferência] O limiar de compactação automática é uma fração da janela (~97%); não é contrato e pode mudar.

## Decisão
1. **Contexto usado** = tokens de prompt (entrada + escrita + leitura de cache) da última mensagem do assistente **do transcript principal** (subagentes têm contexto próprio). Vai para o índice de uso (`state/usage/<sid>.json`, formato v2) junto com o número de compactações e a última delas (`preTokens`, `postTokens`, horário).
2. **Janela** por prefixo de modelo em `config.json: context_windows` (padrão `"claude-": 1000000`; vence o prefixo mais longo). Painel mostra `contexto 45% (452k / 1M) · 2 compactações`. Acima de 85% a linha fica em destaque.
3. **Linha do tempo** (`L`): modal com os eventos da sessão (`events/<sid>.jsonl`) e as ações do ccdeck (`actions.log`: aprovar, recusar, responder), mais recentes primeiro, com quanto tempo a sessão ficou em cada estado e as compactações.

## Alternativas
- **Contar tokens do texto**: impreciso (ADR-0010). Rejeitado.
- **Ler o contexto da tela do Claude Code**: parsing de tela (invariante 2). Rejeitado.

## Consequências
- Mudar o formato do índice força uma reindexação única dos transcripts (em segundo plano).
- Se o Claude Code mudar a janela, basta editar `context_windows`.
