# CONSTITUTION — ccdeck

Invariantes do projeto. Toda mudança (OpenSpec change package, ADR ou PR) precisa respeitá-las; para quebrar uma, primeiro um ADR que a substitua.

## Propósito

ccdeck é um gerenciador de sessões do Claude Code no terminal. Ele organiza várias sessões em paralelo por projeto, isola cada uma num git worktree, mostra o estado de cada sessão em tempo real e permite fechar o app e retomar tudo depois.

## Invariantes

1. **O ccdeck nunca é dono do processo do Claude Code.** As sessões rodam num servidor tmux dedicado (`tmux -L ccdeck`). Fechar, travar ou atualizar o ccdeck não mata nenhuma sessão. (ADR-0002)
2. **Nada de parsing de tela para saber estado.** O estado de uma sessão vem exclusivamente de hooks oficiais do Claude Code. A captura de tela serve só para preview. (ADR-0004)
3. **Não tocar na configuração do usuário.** O ccdeck não edita `~/.claude/settings.json`, `.claude/settings.json` do projeto nem o `~/.tmux.conf`. Hooks entram via `--settings` na linha de comando; tmux usa socket e config próprios. (ADR-0004, ADR-0002)
4. **Uma sessão, um worktree, um branch.** Sessões que pedem isolamento rodam num worktree próprio. Remoção de worktree só acontece com confirmação explícita e nunca com alterações não commitadas, a não ser que o usuário force. (ADR-0003)
5. **Estado local, legível e versionado.** Todo estado persistente fica em arquivos JSON locais com `schema_version`. Nada sai da máquina; não há telemetria. (ADR-0005)
6. **Teclado primeiro.** Toda ação é alcançável por teclado; o mouse é opcional.
7. **Falha visível, nunca silenciosa.** Erro de git, tmux ou hook aparece na barra de status com a causa real.

## Fora de escopo (por enquanto)

- Renderizar o terminal do Claude Code dentro do ccdeck (emulador próprio). Usamos `tmux attach`.
- Windows nativo (WSL funciona por ser Linux).
- Controlar o Claude Code por API/SDK; o ccdeck orquestra a CLI interativa.

## Stack fixa

Rust estável, ratatui + crossterm, serde_json, clap. Dependências externas em runtime: `tmux` ≥ 3.2, `git` ≥ 2.20, `claude` no PATH.
