# ADR-0003 — Um git worktree por sessão

- Status: Aceito
- Data: 2026-10-23

## Contexto
Duas sessões do Claude Code no mesmo diretório editam os mesmos arquivos e brigam pelo índice do git.

## Decisão
Sessões com isolamento ligado (padrão) rodam em `<repo>.worktrees/<slug>` (diretório irmão do repo, para não sujar o repo nem exigir `.gitignore`), num branch novo `ccdeck/<slug>` criado a partir de uma base escolhida. Usamos a CLI `git` via `std::process::Command`, não `libgit2`.

Remoção: `git worktree remove` só se `git status --porcelain` estiver vazio; caso contrário a UI exige confirmação de força. O branch nunca é apagado automaticamente.

## Alternativas
- **`claude --worktree`**: o Claude Code tem suporte próprio a worktrees, mas queremos controlar caminho, branch e ciclo de vida a partir do ccdeck, e mostrar isso na UI. Reavaliar na v0.2.
- **`git2` (libgit2)**: suporte a worktree é incompleto e acrescenta dependência nativa.

## Consequências
- Prefixo de branch configurável (`config.json: branch_prefix`).
- Sessões sem worktree continuam possíveis (ex.: diretório que não é repo).
