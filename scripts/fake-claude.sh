#!/usr/bin/env bash
# Imita o Claude Code para testar o ccdeck sem gastar tokens.
# Fluxo: SessionStart → trabalha → pede permissão (tecla 1 aprova, outra recusa)
#        → Stop com mensagem → espera uma resposta (linha) → Stop → espera Enter → fim.
[ -n "$CCDECK_FAKE_LOG" ] && echo "$@" > "$CCDECK_FAKE_LOG.$CCDECK_SID"
settings=""; sid=""
while [ $# -gt 0 ]; do case "$1" in --settings) settings="$2"; shift 2;; --session-id|--resume) sid="$2"; shift 2;; *) shift;; esac; done
hookcmd=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["hooks"]["Stop"][0]["hooks"][0]["command"])' "$settings")
transcript="${TMPDIR:-/tmp}/fake-claude-$sid.jsonl"
emit(){ printf '{"hook_event_name":"%s","session_id":"%s","transcript_path":"%s"%s}' "$1" "$sid" "$transcript" "$2" | sh -c "$hookcmd"; }
log(){ [ -n "$CCDECK_FAKE_LOG" ] && echo "$1" >> "$CCDECK_FAKE_LOG.$CCDECK_SID.events"; }
echo "fake claude $sid em $(pwd) PORT=${PORT:-} SERVER_PORT=${SERVER_PORT:-}"
log "env PORT=${PORT:-} SERVER_PORT=${SERVER_PORT:-}"
emit SessionStart ""
sleep 1; emit UserPromptSubmit ""; echo "> trabalhando na tarefa..."
echo "linha da sessão $sid" >> comum.txt
emit PreToolUse ',"tool_name":"Bash","tool_use_id":"tu-1","tool_input":{"command":"./mvnw flyway:migrate","description":"migra"}}'
sleep 1
emit PermissionRequest ',"tool_name":"Bash","tool_use_id":"tu-1","tool_input":{"command":"./mvnw flyway:migrate"}'
echo "Permitir executar este comando? ./mvnw flyway:migrate  (1 = sim)"
IFS= read -r -s -n1 key
if [ "$key" = "1" ]; then log "aprovado"; echo "aprovado"; else log "recusado"; echo "recusado"; fi
emit PostToolUse ',"tool_name":"Bash","tool_use_id":"tu-1","tool_input":{}'
sleep 1
emit Stop ',"last_assistant_message":"Pronto. Apliquei a migração V42.\nQuer que eu abra o PR?"'
echo "Pronto. Quer que eu abra o PR?"
IFS= read -r line; log "resposta: $line"; emit UserPromptSubmit ""; echo "recebi: $line"
sleep 1; emit Stop ',"last_assistant_message":"Feito: PR aberto (simulado)."'
IFS= read -r line; emit SessionEnd ""
