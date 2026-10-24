# ADR-0002 — tmux dedicado como backend das sessões

- Status: Aceito; a forma de "entrar" (attach suspendendo a TUI) e o preview por `capture-pane` foram substituídos pelo ADR-0018.
- Data: 2026-10-23

## Contexto
As sessões do Claude Code são interativas (PTY) e precisam sobreviver ao fechamento do gerenciador. Implementar um multiplexador próprio (PTY + emulador VT) é caro e frágil.

## Decisão
Cada sessão é uma sessão tmux num **servidor dedicado** (`tmux -L ccdeck -f <state>/tmux.conf`). O ccdeck:
- cria com `new-session -d -s <id> -c <cwd> -e CCDECK_SID=<id> -- <comando>`;
- mostra o preview com `capture-pane -p -e` convertido de ANSI para ratatui (`ansi-to-tui`);
- entra com `attach -t <id>` (suspendendo a TUI) ou `display-popup` quando já está dentro de tmux;
- usa `Ctrl-q` como `detach-client` só nesse servidor.

## Alternativas
- **PTY próprio + crate `vt100`/`tui-term`**: controle total, mas as sessões morrem com o app e o emulador nunca fica 100% fiel.
- **Usar o tmux do usuário**: colide com o `~/.tmux.conf` e sessões pessoais.

## Consequências
- Sessões persistem entre aberturas do app de graça.
- Dependência de runtime: `tmux` ≥ 3.2 (para `display-popup` e `-e`).
- A config do usuário nunca é tocada (CONSTITUTION, invariante 3).
