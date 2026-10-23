# ccdeck

Gerenciador de sessões do Claude Code no terminal. Organiza várias sessões em paralelo por projeto, isola cada uma num git worktree, mostra em tempo real quem está trabalhando, quem espera aprovação e quem terminou, e deixa você fechar o app e retomar tudo depois.

```
 ccdeck  5 sessões em 2 projetos  2 precisam de você
 Projetos   Atenção (2)        │  migracao-flyway  vendax-crm        ▲ Aguardando aprovação
 / filtrar…                    │  ╭──────────────────────────────────────────────────────────╮
 ▾ vendax-crm  3               │  │ Branch                  Diretório               No estado há │
   ● filtro-clientes     trab. │  │ ccdeck/migracao-flyway  …/vendax-crm.worktrees  7 min       │
   ▲ migracao-flyway    aprov. │  ╰──────────────────────────────────────────────────────────╯
   ◆ fix-login-sso     sua vez │  ╭ Preview ao vivo ─────────────────────────────────────────╮
 ▾ gestor-rq  2                │  │ Permitir executar este comando?                          │
   ● relatorio-cobranca  trab. │  │ ❯ 1. Sim                                                 │
   ‖ refactor-pedidos    susp. │  ╰──────────────────────────────────────────────────────────╯
```

## Requisitos

- macOS ou Linux
- `tmux` ≥ 3.2, `git` ≥ 2.20, Claude Code (`claude`) no PATH
- Rust ≥ 1.88 para compilar

## Instalar

```bash
cargo install --path .
ccdeck doctor     # confere dependências e mostra onde ficam os arquivos
ccdeck            # abre a TUI
```

## Como funciona

- **Sessões vivem num tmux dedicado** (`tmux -L ccdeck`), separado do seu tmux. Fechar o ccdeck não mata nada; `Enter` entra na sessão e `Ctrl-q` volta.
- **Worktree por sessão**: ao criar uma sessão num repositório, o ccdeck cria `<repo>.worktrees/<slug>` num branch `ccdeck/<slug>`.
- **Estado via hooks oficiais**: o ccdeck passa um settings com hooks (`claude --settings`) que chamam `ccdeck hook`. Nada é escrito no seu `~/.claude/settings.json`.
- **Retomada**: cada sessão nasce com `--session-id`; se o processo morrer, `Enter` retoma com `claude --resume <id>` no mesmo diretório.
- **Responder sem entrar**: o painel mostra o pedido de permissão pendente e a última resposta do Claude; `y`/`x` respondem o pedido e `m` envia uma resposta. O `y` só funciona se o pedido estiver na tela há pelo menos 600 ms e ainda for o mesmo.
- **Alterações e colisões**: cada sessão mostra os arquivos que mudou desde a base; se duas sessões do mesmo projeto mexem no mesmo arquivo, aparece `⚠`.
- **Notificação**: quando uma sessão passa a esperar aprovação, termina o turno ou dá erro, você recebe notificação de desktop (`osascript`/`notify-send`) e um sino no terminal.

## Atalhos

| Tecla | Ação |
|---|---|
| `j`/`k`, ↑/↓ | navegar |
| `y` / `x` | aprovar / recusar o pedido pendente sem entrar |
| `m` | responder à sessão sem entrar |
| `D` | diff completo da sessão (`diff_command`) |
| `h`/`l`, ←/→ | recolher/expandir projeto |
| `Enter` | entrar na sessão (retoma se suspensa) |
| `Ctrl-q` | dentro da sessão: voltar ao ccdeck |
| `n` / `N` | nova sessão / novo projeto |
| `a` ou `Tab` | visão Atenção |
| `/` | filtrar |
| `e` | notas (Ctrl-s salva) |
| `r` | renomear |
| `d` | remover sessão (com opção de remover o worktree) |
| `q` | sair |

## Preparo do worktree (`.ccdeck.json`)

Opcional, na raiz do repositório e versionado com o projeto (`ccdeck init-project` gera um exemplo):

```json
{
  "copy": [".env", "config/application-local.yml"],
  "setup": "npm ci",
  "env": { "SPRING_PROFILES_ACTIVE": "dev" },
  "ports": { "base": 8080, "step": 10, "vars": ["PORT", "SERVER_PORT"] }
}
```

- `copy`: arquivos ignorados pelo git copiados para o worktree novo.
- `setup`: roda numa janela `setup` da sessão, em paralelo; o painel mostra rodando/ok/falhou.
- `ports`: cada sessão ganha um slot (1, 2, 3…) e as variáveis valem `base + slot*step + i`. Duas sessões sobem a aplicação sem colidir.
- `env`: variáveis extras. Tudo entra no ambiente do Claude Code e dos comandos que ele roda.

## Configuração

`config.json` (o caminho aparece no `ccdeck doctor`):

```json
{
  "claude_command": "claude",
  "extra_args": [],
  "branch_prefix": "ccdeck/",
  "default_worktree": true,
  "notifications": true,
  "bell": true,
  "approve_keys": ["1"],
  "deny_keys": ["Escape"],
  "diff_command": "git diff {base}"
}
```

`CCDECK_HOME=/algum/dir` coloca config, estado e status num único diretório (útil para testes).

## Documentação

- [CONSTITUTION.md](CONSTITUTION.md) — invariantes
- [docs/SPEC.md](docs/SPEC.md) — especificação da v0.1 e roadmap
- [docs/adr/](docs/adr/) — decisões de arquitetura

## Desenvolvimento

```bash
cargo test            # inclui teste real de git worktree
cargo clippy --all-targets
```
