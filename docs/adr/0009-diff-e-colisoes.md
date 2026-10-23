# ADR-0009 — Alterações por sessão e aviso de colisão

- Status: Aceito
- Data: 2026-10-23

## Contexto
Para revisar uma sessão é preciso saber o que ela mudou. Com várias sessões no mesmo repositório, o conflito só aparece no merge, tarde demais.

## Decisão
- **Alterações** de uma sessão = `git diff --numstat <merge-base(HEAD, base)>` no worktree (inclui commits do branch e mudanças não commitadas) mais arquivos não rastreados (`git ls-files --others --exclude-standard`). Sem worktree, a base é `HEAD`.
- A sessão selecionada é recalculada a cada 3 s; todas as sessões, a cada 15 s, em segundo plano (thread), para não travar a TUI.
- **Colisão**: dentro de um projeto, um arquivo alterado por duas ou mais sessões. A sessão ganha `⚠` na sidebar e o painel lista os arquivos e com quem colidem.
- **Diff completo (`D`)**: suspende a TUI e roda `diff_command` no worktree (padrão `git diff {base}`; dá para usar `lazygit`, `delta`, `tig`…).

## Alternativas
- **`git merge-tree` para prever conflito real**: mais preciso, mas caro e só faz sentido entre pares de branches. Fica para a v0.3; arquivo em comum já é o sinal útil.

## Consequências
- Custo: algumas chamadas de git a cada 15 s por sessão; aceitável até dezenas de sessões.
