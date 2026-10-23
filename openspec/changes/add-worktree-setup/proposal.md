# Change: preparo do worktree

## Why
Worktree novo nasce sem `.env`, sem dependências e com as mesmas portas das outras sessões; o Claude Code tropeça nisso logo no primeiro build ou teste (ADR-0008).

## What Changes
- Arquivo de projeto `.ccdeck.json` com `copy`, `setup`, `env` e `ports`.
- Cópia de arquivos, `setup` numa janela tmux da sessão e portas por slot.
- Painel mostra portas e estado do setup; o formulário de nova sessão mostra o que vai acontecer.
- `ccdeck init-project` gera um exemplo.
- `state.json` vai para `schema_version` 2.

## Impact
- Specs: `worktree-setup`
- Código: novo `project.rs`, `model.rs`, `store.rs` (migração), `app.rs`, `tmux.rs`, `ui.rs`, `main.rs`
- ADR: 0008
