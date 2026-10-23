# ADR-0007 — Responder sem entrar na sessão

- Status: Aceito
- Data: 2026-10-23

## Contexto
Com várias sessões em paralelo, a maior parte do tempo vai em entrar numa sessão só para aprovar um comando ou responder "sim, pode seguir". Também falta saber o que o Claude concluiu sem entrar.

## Decisão
1. **Pedido pendente visível.** O hook passa a guardar no `status/<sid>.json` o pedido de permissão: ferramenta e um resumo do `tool_input` (`command` do Bash, `file_path` de Edit/Write, `url` de WebFetch…). Fonte: `PermissionRequest`; na falta dele, o último `PreToolUse` (que o hook carrega adiante entre eventos).
2. **Aprovar/recusar com `y`/`x`.** (`n` continua sendo "nova sessão".) O ccdeck envia teclas à sessão via `tmux send-keys`. As teclas são configuráveis (`approve_keys`, padrão `["1"]`; `deny_keys`, padrão `["Escape"]`), porque o diálogo pertence ao Claude Code e pode mudar.
3. **Travas contra aprovar no escuro.** O envio só acontece se: (a) a sessão continua em *Aguardando aprovação* no momento da tecla; (b) o pedido mostrado é o mesmo do status atual (mesmo `at`); (c) o pedido ficou na tela por pelo menos 600 ms. Sem pedido conhecido, `y`/`x` ficam desabilitados e a UI manda entrar na sessão.
4. **Resposta rápida (`m`).** Um campo de uma linha envia texto literal (`send-keys -l`) seguido de Enter. Só funciona com a sessão em *Aguardando você*.
5. **Última resposta.** O hook guarda `last_assistant_message` do evento `Stop`. Se ausente, o ccdeck lê o fim do transcript (`transcript_path`, JSONL) e pega o último texto do assistente.

## Alternativas
- **Responder ao `PermissionRequest` pelo próprio hook** (`decision.behavior = allow`): exigiria o hook bloquear esperando o usuário no ccdeck. Viola a regra de que o hook nunca segura o Claude Code. Rejeitado.
- **Ler a tela para achar o diálogo**: frágil (invariante 2). Usamos a tela só como preview.

## Consequências
- Se o Claude Code mudar as teclas do diálogo, basta ajustar o `config.json`.
- A tecla enviada é registrada na linha do tempo da sessão para auditoria.
