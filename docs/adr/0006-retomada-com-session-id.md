# ADR-0006 — Retomada com --session-id e --resume

- Status: Aceito
- Data: 2026-10-23

## Contexto
Precisamos religar uma sessão do ccdeck à conversa certa do Claude Code depois de um reboot.

## Decisão
O ccdeck gera o UUID e inicia com `claude --session-id <uuid> -n "<projeto>/<nome>"`. O hook `SessionStart` confirma (ou corrige) o `session_id` real. Para retomar, `claude --resume <session_id>` no mesmo `cwd`.

## Consequências
- O nome aparece também no `/resume` do próprio Claude Code.
- Se a conversa tiver sido apagada, a retomada falha visivelmente e o usuário pode iniciar uma nova no mesmo worktree.
