# CLAUDE.md — ccdeck

Leia antes de qualquer mudança: `CONSTITUTION.md` (invariantes), `docs/SPEC.md` (escopo e roadmap) e `docs/adr/`.

## Regras de trabalho
- Mudança que contradiz um ADR: escreva primeiro um ADR novo (numeração sequencial) que o substitua.
- Nunca editar `~/.claude/settings.json`, `.claude/` de projetos do usuário nem `~/.tmux.conf` (invariante 3).
- `ccdeck hook` precisa continuar rápido, silencioso e sempre com exit 0.
- Toda mudança de formato do `state.json` sobe `SCHEMA_VERSION` e ganha migração em `store::migrate`.

## Mapa do código
- `main.rs` — CLI (`ccdeck`, `ccdeck hook`, `ccdeck doctor`), criação da sessão `ccdeck-ui` e loop da barra lateral (mouse).
- `app.rs` — estado, reconciliação tmux + hooks, ações e teclado.
- `ui.rs` — desenho com ratatui.
- `hook.rs` — mapeamento evento → estado e geração do `hooks.json`.
- `tmux.rs` — servidor dedicado `-L ccdeck` (`CCDECK_TMUX_SOCKET` troca em testes); painéis marcados e troca por `swap-pane` (ADR-0018).
- `git.rs` — worktrees via CLI.
- `changes.rs` — varredura de alterações em thread e colisões.
- `project.rs` — `.ccdeck.json`: cópia, setup, portas.
- `transcript.rs` — última resposta lida do fim do transcript.
- `usage.rs` — índice incremental de tokens por transcript (+ subagentes) e custo (ADR-0010).
- `events.rs` — histórico de eventos por sessão e tempo em cada estado.
- `summary.rs` — agregados da aba Resumo (funções puras).
- `timeline.rs` — linha do tempo da sessão (eventos + `actions.log` + compactações).
- `openspec.rs` — changes do OpenSpec do repositório (título, progresso, prompt).
- `windows.rs` — janelas auxiliares da sessão (lista do modal `w`).
- `broadcast.rs` — elegíveis e marcação do broadcast `B`.
- `delivery.rs` — entrega (commit/push/PR) e CI via `gh` em segundo plano.
- `store.rs` (migrações de schema), `paths.rs`, `model.rs`, `input.rs`, `notify.rs`.

## Changes
Mudanças novas entram como change package em `openspec/changes/<id>/` (proposal, tasks, specs) e ADR em `docs/adr/`.

## Verificação
`cargo fmt && cargo clippy --all-targets && cargo test`. Para testar o fluxo sem gastar tokens, aponte `claude_command` para `scripts/fake-claude.sh` (e `gh_command` para `scripts/fake-gh.sh`, com `FAKE_GH_DIR`) no `config.json` (com `CCDECK_HOME` num diretório temporário).
