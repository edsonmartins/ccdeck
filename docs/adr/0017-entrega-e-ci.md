# ADR-0017 — Entrega (commit, push, PR) e status do CI (`E`)

- Status: Aceito
- Data: 2026-10-23
- Complementa: ADR-0003, ADR-0009

## Contexto
O fim de uma sessão é sempre igual: commitar o que ficou, subir o branch, abrir o PR e acompanhar o CI. Isso exige sair do ccdeck, ir ao worktree e lembrar o branch base.

## Decisão
1. **Ferramenta**: `git` para commit e push; `gh` para PR e CI, usando a instalação e a autenticação que o usuário já tem (`config.json: gh_command`, padrão `"gh"`). O ccdeck não guarda token nem fala com a API do GitHub diretamente. [confirmado] `gh` 2.97 oferece `pr list --json number,url,headRefName,title,statusCheckRollup`.
2. **`E` abre a entrega** da sessão com worktree: branch → base, arquivos não commitados, commits à frente da base, remoto `origin` e PR aberto (se houver). Ações:
   - **commit**: `git add -A && git commit -m <mensagem>` no worktree (mensagem editável; padrão = nome da sessão);
   - **push**: `git push -u origin <branch>`;
   - **PR**: `gh pr create --base <base> --head <branch> --title <título> --body <notas da sessão>` (título editável).
3. **Toda ação que sai da máquina (push, PR) pede confirmação explícita** mostrando branch, destino (URL do `origin`) e título; o commit também mostra a lista de arquivos antes. As ações rodam em segundo plano e o resultado (ou o erro do `git`/`gh`, inteiro) aparece no modal (invariante 7).
4. **CI**: a cada 60 s, uma chamada `gh pr list --state open --json …` por projeto que tem sessões com branch. O PR é ligado à sessão por `headRefName == branch`. Nada é persistido (o `state.json` não muda). O painel mostra `PR #12 ✓` (tudo verde), `✕ n` (falhas), `… n` (rodando) ou `PR #12` (sem checks).
   - [inferência] Itens do `statusCheckRollup`: `CheckRun` com `status`/`conclusion` e `StatusContext` com `state`. Falha = `conclusion` em FAILURE, TIMED_OUT, CANCELLED, ACTION_REQUIRED, STARTUP_FAILURE ou `state` em FAILURE, ERROR; rodando = `status` diferente de COMPLETED ou `state` PENDING/EXPECTED. Campos desconhecidos são ignorados.
   - Erro do `gh` (sem remoto GitHub, sem login) fica guardado por projeto e aparece no modal de entrega, sem repetir aviso a cada 60 s.

## Alternativas
- **API REST do GitHub com token próprio**: segredo a guardar e mais superfície. Rejeitado.
- **Persistir o número do PR na sessão**: exigiria `schema_version` 3 sem ganho, já que o branch identifica o PR. Rejeitado.
- **Merge pelo ccdeck**: fora do escopo; revisão e merge continuam no GitHub.

## Consequências
- Funciona com qualquer remoto para commit/push; PR e CI só com GitHub e `gh` autenticado.
- [inferência] Repositórios com muitos PRs abertos: `--limit 100` cobre o caso comum; branches de sessões fora desse limite ficam sem status.
