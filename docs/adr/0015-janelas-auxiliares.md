# ADR-0015 — Janelas auxiliares por sessão (`w`)

- Status: Aceito
- Data: 2026-10-23
- Complementa: ADR-0002, ADR-0008

## Contexto
Junto de cada sessão o usuário costuma precisar de um shell no worktree, do servidor da aplicação rodando nas portas da sessão ou de um `tail` de log. Hoje isso exige abrir outro terminal e lembrar o diretório e as portas.

## Decisão
1. `.ccdeck.json` ganha `"windows": {"servidor": "npm run dev", "logs": "tail -f logs/app.log"}` (nome → comando).
2. `w` abre a lista de janelas da sessão: `shell` (o `$SHELL` do usuário), as definidas no projeto e as que já existem na sessão tmux (`claude`, `setup`, `verify`, outras). `Enter` cria a janela, se ainda não existe, no diretório da sessão e com o ambiente dela (portas e `env` do ADR-0008, herdados da sessão tmux), e entra direto nela (`Ctrl-q` volta).
3. `K` fecha a janela selecionada, com confirmação (segundo `K`). A janela `claude` nunca é fechada por aqui; para encerrar a sessão continua valendo `d`.
4. Nomes reservados (`claude`, `setup`, `verify`) no `windows` do projeto são ignorados com aviso.

## Alternativas
- **Splits (panes) na janela do Claude Code**: atrapalham o preview e o envio de teclas (`y`/`x`/`m` miram a janela `claude`). Rejeitado.
- **Abrir um terminal externo**: depende do emulador de terminal e não funciona por SSH. Rejeitado.

## Consequências
- As janelas vivem no tmux do deck: sobrevivem ao fechamento do ccdeck (invariante 1) e morrem com a sessão.
