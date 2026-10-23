# CLAUDE.md — ccdeck

Leia antes de qualquer mudança: `CONSTITUTION.md` (invariantes), `docs/SPEC.md` (escopo e roadmap) e `docs/adr/`.

## Regras de trabalho
- Mudança que contradiz um ADR: escreva primeiro um ADR novo (numeração sequencial) que o substitua.
- Nunca editar `~/.claude/settings.json`, `.claude/` de projetos do usuário nem `~/.tmux.conf` (invariante 3).
- `ccdeck hook` precisa continuar rápido, silencioso e sempre com exit 0.
- Toda mudança de formato do `state.json` sobe `SCHEMA_VERSION` e ganha migração em `store::migrate`.

## Mapa do código
- `main.rs` — CLI (`ccdeck`, `ccdeck hook`, `ccdeck doctor`) e loop da TUI (attach suspende a TUI).
- `app.rs` — estado, reconciliação tmux + hooks, ações e teclado.
- `ui.rs` — desenho com ratatui.
- `hook.rs` — mapeamento evento → estado e geração do `hooks.json`.
- `tmux.rs` — servidor dedicado `-L ccdeck`.
- `git.rs` — worktrees via CLI.
- `changes.rs` — varredura de alterações em thread e colisões.
- `project.rs` — `.ccdeck.json`: cópia, setup, portas.
- `transcript.rs` — última resposta lida do fim do transcript.
- `store.rs` (migrações de schema), `paths.rs`, `model.rs`, `input.rs`, `notify.rs`.

## Changes
Mudanças novas entram como change package em `openspec/changes/<id>/` (proposal, tasks, specs) e ADR em `docs/adr/`.

## Verificação
`cargo fmt && cargo clippy --all-targets && cargo test`. Para testar o fluxo sem gastar tokens, aponte `claude_command` para `scripts/fake-claude.sh` no `config.json` (com `CCDECK_HOME` num diretório temporário).
