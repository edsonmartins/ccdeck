# ADR-0010 — Resumo de uso (tokens, custo, espera)

- Status: Proposto
- Data: 2026-10-23

## Contexto
Com várias sessões em paralelo queremos ver, por sessão, projeto e período: tokens, cache, custo, arquivos modificados, atividade recente e quanto tempo cada sessão ficou parada esperando o usuário.

## Decisão

### Fontes
| Dado | Fonte |
|---|---|
| Tokens de entrada, saída, cache (escrita e leitura), modelo, horário | transcript JSONL da sessão (`transcript_path`, vindo dos hooks) — `message.usage` e `message.model` das linhas do assistente |
| Arquivos modificados | varredura de alterações (ADR-0009) |
| Atividade recente e tempo por estado | novo histórico `state/events/<sid>.jsonl`, uma linha por evento de hook |
| Aprovações e respostas sem entrar | `state/actions.log` (ADR-0007) |

O formato do transcript não é um contrato público do Claude Code. O parser é defensivo: ignora linhas que não entende, nunca falha a TUI, e o primeiro passo da implementação é validar os campos num transcript real. O streaming grava várias linhas para a mesma mensagem: contamos cada `message.id` uma vez, usando a última linha.

### Validação do formato (Claude Code 2.1.309/2.1.310, 2026-10-23)
Feita em 89 transcripts principais e 501 de subagentes desta máquina, olhando só estrutura e números.
- [confirmado] Linhas `type: "assistant"` trazem `timestamp` (ISO 8601 com `Z`), `message.id` (`msg_…`), `message.model`, `message.stop_reason` e `message.usage` com `input_tokens`, `output_tokens`, `cache_read_input_tokens`, `cache_creation_input_tokens` e `cache_creation.{ephemeral_5m_input_tokens, ephemeral_1h_input_tokens}`. Também trazem `usage.speed` (`standard`/`fast`) e `usage.inference_geo`.
- [confirmado] Um `message.id` aparece em até 33 linhas (uma por bloco de conteúdo). No transcript principal o `usage` se repete; nos de subagente há linhas parciais de streaming (`stop_reason: null`, `output_tokens` parcial) seguidas da final. Em todos os 2.763 casos com valores diferentes, a última linha tem o maior `output_tokens` e o `input` não muda. Entre as linhas de um mesmo id podem aparecer linhas `user`, `system` e `attachment`, mas **nunca** outra mensagem de assistente (0 em 32.123 ids com várias linhas). → as linhas de uma mensagem são consecutivas entre as do assistente; o dedupe entre leituras incrementais só precisa lembrar a última mensagem de cada arquivo.
- [confirmado] Linhas com `message.model = "<synthetic>"` (mensagens geradas pelo próprio Claude Code, como a de interrupção) têm uso zero e são ignoradas.
- [confirmado] **Subagentes gravam em arquivos próprios**: `<dir do transcript>/<session_id>/subagents/agent-*.jsonl`. O transcript principal não tem as linhas deles (`isSidechain` é sempre `false` lá). O uso da sessão soma o principal e os subagentes.
- [confirmado] O maior transcript tem 3,4 GB. Antes do parse JSON, a linha é filtrada por substring (`"type":"assistant"`).
- [inferência] O `id` é único por chamada de API (o `requestId` é único por `message.id` em todos os casos vistos).

### Indexação incremental
Para não reler transcripts grandes, `state/usage/<sid>.json` guarda, por arquivo (principal e cada subagente), o offset já lido, e os agregados por hora e modelo (`input`, `output`, `cache_write_5m`, `cache_write_1h`, `cache_read`, `messages`). Cada varredura lê só o que cresceu e para na última linha completa. Para o dedupe entre leituras, o índice guarda a contribuição da última mensagem de cada arquivo: quando chega uma linha nova desse id, a contribuição antiga sai do agregado e a nova entra. O modo rápido entra como modelo `<id>+fast`. Roda numa thread de segundo plano.

### Custo
- `config.json` ganha `prices`: preço por milhão de tokens (entrada, saída, escrita de cache 5 min, escrita de cache 1 h, leitura de cache) por prefixo de id de modelo; vence o prefixo mais longo. O padrão vem da página oficial (`platform.claude.com/docs/en/about-claude/pricing`, lida em 2026-10-23) e pode ser editado. O preço é por mensagem: o Haiku 5.5 tem faixa acima de 100 mil tokens de prompt (`long_context_over` + preços da faixa).
- [inferência] O roteamento só-EUA (`inference_geo: "us"`, 1,1×) não entra no cálculo; nesta máquina todo uso é `global`.
- O custo é exibido como **"equivalente em API"**. Para quem usa assinatura (Pro/Max), não é o valor pago; `cost_display` pode ser `"api"` (padrão) ou `"hidden"`.
- Modelo sem preço configurado: custo mostrado como `?` e aviso no `ccdeck doctor`.

### Telas
- Terceira aba na sidebar: **Projetos | Atenção | Resumo** (`Tab` alterna).
- Resumo: blocos (tokens, custo equivalente, taxa de cache, entrada/saída, arquivos modificados, tempo esperando você), barras de tokens por hora, linha de custo e mensagens, tabela por sessão, atividade recente e arquivos modificados.
- Filtro de período com `p` (hoje / 7 dias / 30 dias) e por projeto com `/`.
- No painel de cada sessão, uma linha compacta com tokens, custo e espera daquela sessão.

## Alternativas
- **OpenTelemetry do Claude Code**: dá métricas oficiais, mas exige coletor rodando e configuração por usuário. Pode virar uma segunda fonte depois.
- **Estimar tokens pelo texto**: impreciso. Rejeitado.

## Consequências
- Tudo local (invariante 5): nenhum dado de uso sai da máquina.
- Se o formato do transcript mudar, o resumo degrada (mostra `—`) sem afetar o resto do ccdeck.
