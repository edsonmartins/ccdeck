# Change: responder sem entrar na sessão

## Why
Entrar e sair de cada sessão só para aprovar um comando ou dizer "pode seguir" é o maior desperdício de tempo com várias sessões em paralelo (ADR-0007).

## What Changes
- O hook passa a registrar o pedido de permissão pendente (ferramenta + resumo do `tool_input`) e a última resposta do assistente.
- O painel da sessão mostra **Pedido pendente** e **Última resposta**.
- Teclas `y` (aprovar), `x` (recusar) e `m` (resposta rápida), com travas contra aprovar no escuro.
- Configuração `approve_keys` / `deny_keys`.

## Impact
- Specs: `session-interaction`
- Código: `hook.rs`, `model.rs` (StatusRecord), `tmux.rs` (send-keys), `app.rs`, `ui.rs`, novo `transcript.rs`
- ADR: 0007
