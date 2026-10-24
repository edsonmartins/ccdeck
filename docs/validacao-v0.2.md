# Validação da v0.2 no ambiente real

- Data: 2026-10-23
- Máquina: macOS (Darwin 27.0.0), Apple Silicon
- Versões: `tmux 3.8` (instalado via Homebrew para esta validação), `git 2.54.0 (Apple Git-157)`, `claude 2.1.309` no início, `2.1.310` durante os testes (atualização automática)
- Método: repositório descartável e `CCDECK_HOME` temporário no scratchpad; TUI real dirigida por `tmux send-keys`; `claude_command` apontando para um wrapper que repassa os argumentos ao `claude` real e soma ao `--settings` um hook que grava o JSON cru de cada evento (o `hooks.json` do ccdeck não foi alterado). `~/.claude/settings.json`, `.claude/` de outros projetos e `~/.tmux.conf` não foram tocados.

Legenda: `[confirmado]` = observado nesta validação, com evidência; `[inferência]` = deduzido, não observado diretamente.

## 1. Build e dependências

- [confirmado] `cargo build --release` sem avisos.
- [confirmado] `ccdeck doctor` detecta a falta do tmux (`FALTA tmux ≥ 3.2`), mas **saía com código 0**. Corrigido: agora sai com erro (`dependências faltando: …`, exit 1). Commit `bc1662c`.

## 2. Hooks e nomes de evento

- [confirmado] Os eventos observados batem com `hook::EVENTS`: `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PermissionRequest`, `Notification`, `PostToolUse`, `Stop`, `SessionEnd`. Sequência real de um turno com aprovação:

  ```
  SessionStart → UserPromptSubmit → PreToolUse(Bash) → PermissionRequest(Bash)
  → Notification(permission_prompt) → PostToolUse(Bash) → Stop
  ```
- [confirmado] `Notification` traz `notification_type` e `message`: `permission_prompt` / "Claude needs your permission"; `idle_prompt` / "Claude is waiting for your input", 60 s depois de um `Stop`.
- [confirmado] `SessionEnd` chega no `/exit` com `reason: "prompt_input_exit"`; status `ended`.
- [inferência] `StopFailure` não foi provocado (exige erro de API). O nome existe no binário 2.1.310, junto com `PostToolUseFailure`, `PermissionDenied`, `SubagentStop`, `PreCompact` e outros.
- [confirmado] Campos extras que o ccdeck ainda não usa: `prompt_id`, `permission_mode`, `effort`, `session_title` (o valor do `-n`), `model` (no `SessionStart`), `source` (`startup`/`resume`), `duration_ms` (`PostToolUse`).

## 3. `PermissionRequest`

- [confirmado] Traz `tool_name` e `tool_input` (`{"command":"mkdir pasta-y","description":"Create directory pasta-y"}`) e também `permission_suggestions`.
- [confirmado] **Não traz `tool_use_id`**. Só `PreToolUse` e `PostToolUse` trazem. O código esperava o campo e caía num id sintético `t<nanos>`. Corrigido: o pedido herda o id do `PreToolUse` da mesma chamada (mesma ferramenta e mesmo resumo). Fixture do teste ajustada ao formato real. Commit `668d1ea`.
  - [inferência] A trava do `y`/`x` usa `at`, não o id, então o id sintético não causava aprovação errada; só perdia a ligação com o `tool_use_id`.

Trecho de `status/<sid>.json` durante o pedido (antes da correção):

```json
{"status":"approval","event":"Notification · permission_prompt",
 "pending":{"tool":"Bash","summary":"mkdir pasta-y","id":"t1792799605474698000"}}
```

## 4. `Stop`

- [confirmado] Traz `last_assistant_message` (`"OK"` nos três turnos testados), e o ccdeck grava em `last_message`. Também traz `stop_hook_active`, `background_tasks` e `session_crons`.

## 5. Teclas de aprovação e recusa

Diálogo atual (2.1.310):

```
Do you want to proceed?
❯ 1. Yes
  2. Yes, and always allow access to <worktree> from this project
  3. Yes, and switch to auto mode · auto mode handles these prompts for you
  4. No
Esc to cancel · Tab to amend
```

- [confirmado] `approve_keys = ["1"]` aprova: `y` na TUI, a ferramenta rodou (`pasta-y` criada), seguiram `PostToolUse` e `Stop`, e o `actions.log` registrou `aprovar  Bash: mkdir pasta-y`.
- [confirmado] `deny_keys = ["Escape"]` recusa: `x` na TUI, a ferramenta não rodou e o `actions.log` registrou `recusar`. A tela mostra `Interrupted · What should Claude do instead?`.
- [confirmado] **Bug: depois da recusa nenhum hook dispara**, nem `Stop`, `PostToolUseFailure`, `PermissionDenied` ou `idle_prompt` (observado por mais de 5 min). Recusar com `4. No` tem o mesmo efeito. A sessão fica presa em *Aguardando aprovação*, e com isso o `m` (resposta rápida) fica bloqueado.
- [confirmado] O mesmo acontece com `Esc` no meio de um turno: a sessão fica presa em *Trabalhando*. **Interrupção não gera evento de hook nesta versão.** → pendente de decisão (ver §8).
- [confirmado] `m` (resposta rápida) envia o texto e dispara `UserPromptSubmit`.

## 6. Flags de linha de comando (`App::claude_command`)

- [confirmado] `--session-id <uuid>`: o `session_id` do `SessionStart` é igual ao UUID gerado e o transcript é `~/.claude/projects/<cwd-codificado>/<uuid>.jsonl`.
- [confirmado] `-n "<projeto>/<nome>"` aparece como `session_title` nos hooks e no rodapé do Claude Code.
- [confirmado] `--settings <hooks.json>` soma-se às settings do usuário: os hooks do ccdeck disparam e o `defaultMode` do usuário continua valendo.
- [confirmado] `--resume <uuid>`: com a sessão tmux morta a TUI mostra *Suspensa*; `Enter` recria a sessão com `--resume`, chega um `SessionStart` com o mesmo `session_id` e `source: "resume"`, e o histórico aparece na tela.
- [confirmado] Modo de permissão `default` no formulário não passa `--permission-mode`, então vale o `defaultMode` das settings do usuário (`auto` nesta máquina), e nenhum pedido de aprovação aparece. → pendente de decisão (ver §8).

## 7. Outros achados

- [confirmado] **Diálogo de confiança na pasta**: o primeiro worktree do repositório abriu "Is this a project you trust?". Antes da resposta nenhum hook dispara e a sessão fica em *Iniciando*. O painel agora sugere entrar na sessão (commit `a7ce1cd`).
  - [inferência] Os worktrees seguintes do mesmo repositório não abriram o diálogo; a confiança parece herdada pelo repositório git, não pela pasta.
- [confirmado] O Claude Code avisa `tmux focus-events off`. O servidor `-L ccdeck` agora liga `focus-events` no próprio `tmux.conf` (commit `fe5ce3c`).
- [confirmado] **`CCDECK_HOME` herdado do servidor tmux**: as sessões herdam o ambiente global do `tmux -L ccdeck`. Com duas instâncias de `CCDECK_HOME` diferentes, os hooks da segunda gravaram no diretório da primeira (`tmux show-environment -g CCDECK_HOME`). O comando no `hooks.json` agora fixa `CCDECK_HOME=<dir>` ou `env -u CCDECK_HOME` (commit `2c1be81`).
- [confirmado] O `ccdeck hook` leva cerca de 5,7 ms por chamada (100 chamadas em 0,57 s, contando a criação do processo), sem saída e com exit 0, mesmo com o histórico de eventos.
- [confirmado] Aviso de colisão funcionou com sessões reais: duas sessões criaram `aprovado.txt` e as duas ganharam `⚠`.

## 8. Decisões tomadas depois da validação

1. **Estado depois de recusa ou interrupção** → ADR-0011 (invariante 2 passa a aceitar estado vindo de ações do próprio ccdeck; parsing de tela continua proibido). Change `fix-state-after-interrupt`.
   - [confirmado] Depois do `x`, `status/<sid>.json` fica `{"status":"yours","event":"ccdeck · recusado","pending":null}`. O `m` enviado logo em seguida dispara `UserPromptSubmit` → `Stop` com `last_assistant_message: "ENTENDIDO"`, e a pasta pedida não foi criada.
   - [confirmado] Depois da correção do §3, o `pending.id` do mesmo pedido é o `tool_use_id` real (`toolu_011Z…`).
   - Interrupção com `Esc` dentro da sessão continua sem hook. O painel mostra "sem eventos há N min" em *Trabalhando* parado há 5 min ou mais (só aviso). [inferência] Não foi esperado o tempo do aviso em sessão real; coberto por teste unitário.
2. **Modo `default`** → aparece na UI como "padrão das suas settings"; o comportamento (não passar `--permission-mode`) foi mantido.
