# ADR-0011 — Estado também a partir de ações do próprio ccdeck

- Status: Aceito
- Data: 2026-10-23
- Altera: invariante 2 da CONSTITUTION; complementa ADR-0004 e ADR-0007

## Contexto
A validação no Claude Code 2.1.310 (`docs/validacao-v0.2.md`, §5) mostrou que recusar um pedido de permissão (`Escape` ou `4. No`) e interromper um turno com `Esc` **não disparam nenhum hook** [confirmado]: nem `Stop`, `PostToolUseFailure`, `PermissionDenied` ou `Notification(idle_prompt)`, mesmo depois de 5 min. A sessão fica presa em *Aguardando aprovação* ou *Trabalhando*, e a resposta rápida (`m`) fica bloqueada.

A invariante 2 dizia que o estado vem *exclusivamente* de hooks. Ela existe para proibir parsing de tela, não ações conhecidas do próprio ccdeck.

## Decisão
1. A invariante 2 passa a ser: **o estado vem de hooks oficiais do Claude Code ou de ações que o próprio ccdeck executou; nunca de parsing de tela.**
2. Depois de enviar `deny_keys` com sucesso (`x`), o ccdeck grava em `status/<sid>.json` um registro `Aguardando você` com evento `ccdeck · recusado`, carregando adiante `last_tool`, `last_message`, `transcript_path` e `claude_session_id`. A gravação só acontece se o status no disco ainda é o mesmo que motivou a recusa (mesmo `at`). Se um hook chegou no meio, ele vence.
3. Aprovar (`y`) não grava nada: o Claude Code segue e os hooks (`PostToolUse`, `Stop`) chegam normalmente [confirmado].
4. Para interrupções feitas dentro da sessão, que o ccdeck não vê, o painel mostra "sem eventos há N min" quando a sessão está em *Trabalhando* há 5 min ou mais sem evento novo. É só um aviso: o estado não muda.

## Alternativas
- **Marcar como *Aguardando você* ao voltar do attach (Ctrl-q)**: adivinha o que aconteceu lá dentro; erra quando o usuário só olhou e saiu. Rejeitado.
- **Mandar um prompt depois da recusa para forçar um `Stop`**: gasta tokens e muda a conversa. Rejeitado.
- **Ler a tela atrás de "Interrupted"**: viola o espírito da invariante 2. Rejeitado.

## Consequências
- O `x` deixa a sessão pronta para `m` na hora.
- O registro sintético aparece no `status/<sid>.json` com o evento `ccdeck · recusado`, então dá para distinguir de um hook.
- Se uma versão futura do Claude Code passar a emitir hook na recusa, o hook sobrescreve o registro sintético e nada quebra.
