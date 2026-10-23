# ADR-0004 — Estado das sessões via hooks do Claude Code

- Status: Aceito
- Data: 2026-10-23

## Contexto
Precisamos saber se cada sessão está trabalhando, esperando aprovação, esperando o usuário ou terminou. Ler a tela (procurar spinners ou textos) quebra a cada versão do Claude Code.

## Decisão
O ccdeck gera um arquivo de settings próprio com hooks `type: command` apontando para `ccdeck hook` e o passa em cada sessão com `claude --settings <arquivo>`. Eventos usados: `SessionStart`, `UserPromptSubmit`, `PreToolUse`, `PostToolUse`, `PermissionRequest`, `Notification`, `Stop`, `StopFailure`, `SessionEnd`.

O subcomando `ccdeck hook`:
1. lê o JSON do stdin (`session_id`, `hook_event_name`, `cwd`, `notification_type`, `message`...);
2. identifica a sessão do ccdeck pela variável `CCDECK_SID` (definida no ambiente da sessão tmux); fora do ccdeck, sai com 0 sem fazer nada;
3. grava `status/<sid>.json` de forma atômica (arquivo temporário + rename);
4. sempre retorna 0 rapidamente, para nunca bloquear o Claude Code.

## Alternativas
- **Editar `~/.claude/settings.json`**: afeta todas as sessões do usuário, inclusive fora do ccdeck. Rejeitado (invariante 3).
- **Hook `http` para um servidor local**: exige o ccdeck rodando para não perder eventos; arquivo funciona com o app fechado.

## Consequências
- Ao reabrir o app, o último estado de cada sessão já está no disco.
- Hooks configurados pelo usuário continuam valendo; settings passados via `--settings` se somam aos demais.
