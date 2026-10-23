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

### Indexação incremental
Para não reler transcripts grandes, `state/usage/<sid>.json` guarda o offset já lido e os agregados por hora (`tokens_in`, `tokens_out`, `cache_write`, `cache_read`, `messages`, por modelo). Cada varredura lê só o que cresceu. Roda na mesma thread de segundo plano das alterações.

### Custo
- `config.json` ganha `prices`: preço por milhão de tokens (entrada, saída, escrita e leitura de cache) por prefixo de id de modelo. O padrão vem da página oficial de preços na data deste ADR e pode ser editado.
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
