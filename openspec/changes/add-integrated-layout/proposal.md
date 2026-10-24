# Change: layout integrado (árvore à esquerda, sessão à direita)

## Why
Entrar numa sessão troca a tela inteira; o usuário quer a árvore sempre visível e a sessão viva ao lado, com mouse (ADR-0018).

## What Changes
- `ccdeck ui` (protótipo): sessão tmux `ccdeck-ui` com barra lateral + painel da sessão.
- Painel do Claude Code marcado por sessão; troca por `swap-pane` com painel de espera.
- Mouse na barra (clique, roda); `Ctrl-q` volta à barra; zoom para modais.

## Impact
- Specs: `integrated-layout`
- Código: `tmux.rs`, `app.rs`, `ui.rs`, `main.rs`
- ADR: 0018 (Proposto)
