# SPEC — ccdeck v0.1

## 1. Problema

Rodar várias sessões do Claude Code em abas ou splits de terminal vira bagunça: não dá para saber qual sessão está trabalhando, qual está parada esperando aprovação e qual já terminou; várias sessões no mesmo repositório pisam umas nas outras; e fechar o terminal perde o contexto de qual sessão era o quê.

## 2. Objetivos da v0.1

| # | Objetivo | Critério de aceite |
|---|----------|--------------------|
| O1 | Agrupar sessões por projeto | Sidebar em árvore Projeto → Sessões, com nome, branch e estado. Projetos podem ser recolhidos. |
| O2 | Isolar sessões com git worktree | Ao criar uma sessão com "worktree" marcado, o ccdeck cria `<repo>.worktrees/<slug>` num branch `ccdeck/<slug>` a partir de uma base escolhida, e inicia o Claude Code lá. |
| O3 | Estado em tempo real + notificação | Cada sessão mostra um destes estados: Iniciando, Trabalhando, Aguardando aprovação, Aguardando você, Erro, Encerrada, Suspensa. Mudanças para *Aguardando aprovação*, *Aguardando você* ou *Erro* geram notificação de desktop e um sino no terminal, e a sessão sobe para a fila "Atenção". |
| O4 | Persistir e retomar | Ao reabrir o ccdeck, sessões cujo processo ainda vive (tmux) aparecem como estavam; sessões mortas aparecem como Suspensa e um Enter as retoma com `claude --resume <id>` no mesmo diretório. |

## 3. Conceitos

- **Projeto**: um diretório (normalmente a raiz de um repositório git) com nome e cor.
- **Sessão**: uma instância do Claude Code ligada a um projeto, com nome, diretório de trabalho (o repo ou um worktree), branch, `claude_session_id` (UUID), notas livres e histórico de estados.
- **Deck**: o servidor tmux dedicado `tmux -L ccdeck` que hospeda todas as sessões.

## 4. Fluxos

### 4.1 Nova sessão (`n`)
1. Usuário escolhe nome, base (`main` por padrão), se quer worktree e o modo de permissão inicial.
2. Se worktree: `git -C <repo> worktree add -b ccdeck/<slug> <repo>.worktrees/<slug> <base>`.
3. ccdeck gera um UUID e grava `~/.local/state/ccdeck/hooks.json` (uma vez; mesmo arquivo para todas as sessões).
4. `tmux -L ccdeck new-session -d -s <id-curto> -c <cwd> -e CCDECK_SID=<id> -- claude --session-id <uuid> -n "<projeto>/<nome>" --settings <hooks.json> [--permission-mode X]`.
5. Estado inicial: Iniciando.

### 4.2 Entrar numa sessão (`Enter`)
- Fora do tmux: o ccdeck suspende a TUI, executa `tmux -L ccdeck attach -t <id>` e volta quando o usuário sai com `Ctrl-q` (atalho configurado só no servidor do deck).
- Dentro de tmux: abre um popup (`display-popup`) com o attach, para não aninhar sessões.

### 4.3 Estados via hooks
O `hooks.json` registra `ccdeck hook` como comando em todos os eventos abaixo. O subcomando lê o JSON do stdin, pega `CCDECK_SID` do ambiente e grava `~/.local/state/ccdeck/status/<sid>.json` de forma atômica.

| Evento do Claude Code | Estado no ccdeck |
|---|---|
| `SessionStart` | Aguardando você (e atualiza `claude_session_id`) |
| `UserPromptSubmit`, `PreToolUse`, `PostToolUse` | Trabalhando |
| `PermissionRequest`, `Notification` com `permission_prompt` | Aguardando aprovação |
| `Notification` com `idle_prompt` / `agent_needs_input` | Aguardando você |
| `Stop` | Aguardando você (turno concluído) |
| `StopFailure` | Erro |
| `SessionEnd` | Encerrada |

A TUI observa o diretório `status/` (polling de 500 ms) e reconcilia com `tmux list-sessions` a cada 2 s.

### 4.4 Retomar
Na abertura, para cada sessão salva: se a sessão tmux existe → mantém o último estado; se não existe → Suspensa. `Enter` numa Suspensa recria a sessão tmux com `claude --resume <claude_session_id>` no mesmo `cwd`.

### 4.5 Remover (`d`)
Confirmação com três opções: só encerrar (mantém worktree e branch), encerrar e remover worktree (bloqueado se houver mudanças não commitadas), cancelar.

## 5. Telas
Ver o protótipo navegável das telas (canvas de Design). Resumo:
- **Principal**: sidebar (projetos e sessões, com ícone de estado) | painel da sessão (cabeçalho com branch, worktree, estado, tempo no estado; preview ao vivo do terminal; notas) | barra de atalhos.
- **Atenção** (`a`): lista plana só com sessões que precisam de você, ordenadas por tempo de espera.
- **Nova sessão / Novo projeto**: formulário modal.
- **Confirmar remoção**, **Notas**, **Ajuda**.

## 6. Atalhos

| Tecla | Ação |
|---|---|
| `j`/`k` ou ↑/↓ | navegar |
| `h`/`l` ou ←/→ | recolher/expandir projeto |
| `Enter` | entrar na sessão (ou retomar) |
| `n` / `N` | nova sessão / novo projeto |
| `a` | alternar visão Atenção |
| `Tab` | Projetos → Atenção → Resumo |
| `p` | período do Resumo (hoje / 7 / 30 dias) |
| `e` | editar notas |
| `r` | renomear |
| `d` | remover |
| `/` | filtrar |
| `?` | ajuda |
| `q` | sair (sessões continuam rodando) |
| `Ctrl-q` (dentro da sessão) | voltar ao ccdeck |

## 7. Arquivos

```
~/.config/ccdeck/config.json        preferências (worktree padrão, notificações, comando do claude)
~/.local/share/ccdeck/state.json    projetos e sessões (schema_version)
~/.local/state/ccdeck/hooks.json    settings com hooks, passado via --settings
~/.local/state/ccdeck/status/*.json último evento de cada sessão
~/.local/state/ccdeck/tmux.conf     config do servidor tmux do deck
```
No macOS os caminhos seguem `directories` (`~/Library/Application Support/...`).

## 8. Requisitos não funcionais
- Abrir em < 150 ms com 50 sessões; consumo ocioso < 1% de CPU.
- Funciona em macOS e Linux, em qualquer terminal com 256 cores; respeita `NO_COLOR`.

## 9. Roadmap
- **v0.1** — escopo acima (O1–O4).
- **v0.2** — changes em `openspec/changes/`:
  - `add-quick-reply` — pedido pendente, aprovar/recusar com `y`/`x`, resposta rápida `m`, última resposta (ADR-0007)
  - `add-worktree-setup` — `.ccdeck.json` com cópia de arquivos, `setup` e portas por sessão (ADR-0008)
  - `add-diff-and-collisions` — alterações por sessão, aviso de colisão, diff completo com `D` (ADR-0009)
  - `fix-state-after-interrupt` — estado depois de recusa/interrupção, que não disparam hook (ADR-0011)
  - `add-usage-summary` — aba Resumo: tokens, custo equivalente em API, cache, tempo esperando você, atividade e arquivos (ADR-0010)
- **v0.3** — changes em `openspec/changes/` (em andamento):
  - `add-context-timeline` — contexto usado, compactações e linha do tempo `L` (ADR-0012)
- **v0.3 (restante)** — sessão a partir de change do OpenSpec, templates por projeto, verificação por sessão (`t`), entrega com commit/push/PR via `gh` e status do CI, linha do tempo e custo/contexto por sessão, janelas auxiliares, broadcast de prompt.
