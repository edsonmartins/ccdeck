# Change: modelos de sessão e sessão a partir de change do OpenSpec

## Why
Prompts iniciais repetidos e sessões que nascem para implementar um change do OpenSpec (ADR-0014).

## What Changes
- `.ccdeck.json`: `templates` e `openspec_prompt`.
- Novo `openspec.rs`: lista changes com título e progresso.
- Formulário de nova sessão: campo Modelo; preenche prompt, nome e modo; aviso se o change não está na base.

## Impact
- Specs: `session-starters`
- Código: `project.rs`, novo `openspec.rs`, `git.rs`, `input.rs`, `app.rs`, `ui.rs`
- ADR: 0014
