# ADR-0018 — Layout integrado: árvore à esquerda, sessão à direita (no tmux)

- Status: Proposto (protótipo em `ccdeck ui`)
- Data: 2026-10-24
- Substituiria, se aceito: a parte "entrar = attach que suspende a TUI" do ADR-0002 e o item "Renderizar o terminal dentro do ccdeck" de "Fora de escopo" da CONSTITUTION (que passaria a dizer "emulador de terminal próprio").

## Contexto
No uso real, entrar numa sessão troca a tela inteira: a árvore some e volta só com `Ctrl-q`. O usuário quer a árvore de projetos e sessões sempre visível à esquerda, a sessão selecionada viva à direita, e clicar com o mouse para trocar (referência: layout do better-agent-terminal, app Tauri com xterm.js).

## Decisão (proposta)
1. `ccdeck ui` cria (ou reaproveita) a sessão tmux `ccdeck-ui` no servidor do deck, com uma janela de dois painéis: à esquerda o ccdeck em modo barra lateral; à direita o painel da sessão selecionada.
2. A sessão é mostrada **movendo o painel real do Claude Code** para a direita com `tmux swap-pane`; quem ocupa o lugar dele na janela de origem é um painel de espera (`@ccdeck_placeholder`). Antes de mostrar outra, a anterior volta para casa. Só uma sessão fica "emprestada" por vez, e qual é fica gravado em `@ccdeck_shown` na sessão `ccdeck-ui` (sobrevive a um crash do ccdeck).
3. O painel do Claude Code é marcado com `@ccdeck_session=<nome>`: `send-keys`, `capture-pane` e a detecção de sessão viva passam a achar o painel pela marca, onde quer que ele esteja.
4. Mouse: a barra lateral pede eventos de mouse (crossterm); clique seleciona e mostra, roda do mouse navega. Clicar no painel da direita digita no Claude Code (mouse do tmux). `Ctrl-q` na direita volta para a barra; na barra, desconecta (tudo continua rodando).
5. Modais e telas largas (nova sessão, Resumo, diff) dão zoom no painel da barra enquanto estão abertos.

## Validação do mecanismo (tmux 3.8)
- [confirmado] `swap-pane -d` entre sessões diferentes leva o painel com o processo e as opções de painel (`@ccdeck_session`).
- [confirmado] Se o painel mostrado morre, `join-pane` devolve o painel de espera para a direita e a sessão de origem, vazia, some sozinha.
- [confirmado] `if-shell -F "#{==:#{session_name},…}"` permite `Ctrl-q` diferente dentro de `ccdeck-ui`.

## Protótipo (2026-10-24)
- [confirmado] `ccdeck ui` num servidor isolado (`CCDECK_TMUX_SOCKET`) com o claude falso: criar projeto e sessões pela barra (modal com zoom), sessão nova aparece à direita com foco; clique de mouse (sequência SGR) na barra troca a sessão mostrada e devolve a anterior; `y` na barra aprova a sessão emprestada (alvo pela marca); sessão mostrada que morre libera a direita e vira *Suspensa*; `Enter` retoma e mostra; `Ctrl-q` na direita volta à barra, na barra desconecta; `q` devolve o painel e fecha `ccdeck-ui`.
- [confirmado] Risco: fechar `ccdeck-ui` à força (`tmux kill-session`, `kill-server`) com uma sessão emprestada mata o Claude Code dela, porque o painel está lá. O `q` do ccdeck devolve antes de fechar.

## Alternativas
- **Emulador dentro do ratatui (`vt100`/`tui-term`)**: fidelidade parcial com a TUI do Claude Code. Rejeitado (ADR-0001).
- **App gráfico (Tauri + xterm.js)**: reescrita e perda do uso por SSH. Rejeitado por ora.

## Consequências
- Invariante 1 mantida: as sessões continuam no tmux; fechar o ccdeck devolve o painel emprestado para casa.
- Invariante 2 mantida: o terminal é o próprio tmux; nada de parsing.
- O preview por `capture-pane` deixa de ser necessário no modo integrado.
