#!/usr/bin/env bash
# Imita o Claude Code para testar o ccdeck sem gastar tokens.
# Segue o comportamento do Claude Code 2.1.310 (docs/validacao-v0.2.md):
#   SessionStart → trabalha → PreToolUse → PermissionRequest (sem tool_use_id)
#   → Notification(permission_prompt). Tecla 1 aprova (PostToolUse + Stop);
#   qualquer outra recusa e, como no real, NÃO dispara hook nenhum.
#   Depois espera uma linha de resposta → Stop → espera Enter → SessionEnd.
# O transcript recebe linhas de assistente com usage (ADR-0010), incluindo
# linhas parciais de streaming e uma `<synthetic>`.
[ -n "$CCDECK_FAKE_LOG" ] && echo "$@" > "$CCDECK_FAKE_LOG.$CCDECK_SID"
settings=""; sid=""
while [ $# -gt 0 ]; do case "$1" in --settings) settings="$2"; shift 2;; --session-id|--resume) sid="$2"; shift 2;; *) shift;; esac; done
hookcmd=$(python3 -c 'import json,sys;print(json.load(open(sys.argv[1]))["hooks"]["Stop"][0]["hooks"][0]["command"])' "$settings")
transcript="${TMPDIR:-/tmp}/fake-claude-$sid.jsonl"
emit(){ printf '{"hook_event_name":"%s","session_id":"%s","transcript_path":"%s"%s}' "$1" "$sid" "$transcript" "$2" | sh -c "$hookcmd"; }
log(){ [ -n "$CCDECK_FAKE_LOG" ] && echo "$1" >> "$CCDECK_FAKE_LOG.$CCDECK_SID.events"; }
# assistant <id> <model> <input> <output> <cache_read> <cache_write_1h> <stop_reason>
assistant(){
  printf '{"type":"assistant","timestamp":"%s","message":{"id":"%s","model":"%s","role":"assistant","stop_reason":%s,"content":[{"type":"text"}],"usage":{"input_tokens":%s,"output_tokens":%s,"cache_read_input_tokens":%s,"cache_creation_input_tokens":%s,"cache_creation":{"ephemeral_5m_input_tokens":0,"ephemeral_1h_input_tokens":%s},"speed":"standard"}}}\n' \
    "$(date -u +%Y-%m-%dT%H:%M:%S.000Z)" "$1" "$2" "$7" "$3" "$4" "$5" "$6" "$6" >> "$transcript"
}
echo '{"type":"user","timestamp":"'"$(date -u +%Y-%m-%dT%H:%M:%S.000Z)"'"}' >> "$transcript"
echo "fake claude $sid em $(pwd) PORT=${PORT:-} SERVER_PORT=${SERVER_PORT:-}"
log "env PORT=${PORT:-} SERVER_PORT=${SERVER_PORT:-}"
emit SessionStart ""
sleep 1; emit UserPromptSubmit ""; echo "> trabalhando na tarefa..."
echo "linha da sessão $sid" >> comum.txt
n=$(date +%s)
assistant "msg_${n}_1" claude-opus-5-5 2 5 0 12000 null
assistant "msg_${n}_1" claude-opus-5-5 2 84 0 12000 '"tool_use"'
emit PreToolUse ',"tool_name":"Bash","tool_use_id":"tu-1","tool_input":{"command":"./mvnw flyway:migrate","description":"migra"}'
sleep 1
emit PermissionRequest ',"tool_name":"Bash","tool_input":{"command":"./mvnw flyway:migrate","description":"migra"},"permission_suggestions":[]'
emit Notification ',"notification_type":"permission_prompt","message":"Claude needs your permission"'
echo "Permitir executar este comando? ./mvnw flyway:migrate  (1 = sim)"
IFS= read -r -s -n1 key
if [ "$key" = "1" ]; then
  log "aprovado"; echo "aprovado"
  emit PostToolUse ',"tool_name":"Bash","tool_use_id":"tu-1","tool_input":{}'
  sleep 1
  assistant "msg_${n}_2" claude-opus-5-5 2 40 12000 300 '"end_turn"'
  emit Stop ',"last_assistant_message":"Pronto. Apliquei a migração V42.\nQuer que eu abra o PR?"'
  echo "Pronto. Quer que eu abra o PR?"
else
  log "recusado"; echo "Interrupted · What should Claude do instead?"
  assistant "msg_${n}_x" '<synthetic>' 0 0 0 0 null
fi
IFS= read -r line; log "resposta: $line"; emit UserPromptSubmit ""; echo "recebi: $line"
sleep 1
assistant "msg_${n}_3" claude-sonnet-5-5 2 10 12300 50 '"end_turn"'
emit Stop ',"last_assistant_message":"Feito: PR aberto (simulado)."'
IFS= read -r line; emit SessionEnd ""
