# ADR-0013 — Verificação por sessão (`t`)

- Status: Aceito
- Data: 2026-10-23
- Complementa: ADR-0008

## Contexto
Antes de revisar ou entregar o que uma sessão fez, o usuário quer rodar os testes do projeto no worktree dela, sem entrar na sessão e sem parar o Claude Code.

## Decisão
1. O `.ccdeck.json` ganha `"verify": "<comando>"` (ex.: `"./mvnw -q test"`, `"npm test"`). É do projeto, versionado pelo time, como o `setup` (ADR-0008).
2. `t` roda o comando no worktree, numa janela tmux `verify` da própria sessão (o mesmo mecanismo do `setup`): saída em `state/verify/<sid>.log`, código de saída em `state/verify/<sid>.exit`, horário de início em `state/verify/<sid>.start`. Se uma verificação já está rodando, `t` não dispara outra; a anterior concluída é substituída.
3. O painel mostra `verificação: rodando… / ok (12 s) / falhou (código 1) / interrompida`. Quando termina, há aviso na barra de status e notificação de desktop (se ligada). `T` abre o log com `less`.
4. Exige a sessão tmux viva (a janela é criada nela). Sessão suspensa: a UI pede para retomar antes.

## Alternativas
- **Pedir ao Claude Code para rodar os testes**: gasta tokens e ocupa a sessão. Rejeitado como padrão (o usuário sempre pode pedir via `m`).
- **Rodar fora do tmux, num processo filho do ccdeck**: morreria ao fechar o ccdeck (invariante 1). Rejeitado.

## Consequências
- O resultado sobrevive ao fechamento do ccdeck (arquivos + janela tmux).
- [inferência] Testes que sobem servidores usam as portas da sessão (ADR-0008), porque a janela herda o ambiente da sessão tmux.
