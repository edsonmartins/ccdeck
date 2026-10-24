#!/usr/bin/env bash
# Imita o gh para testar a entrega do ccdeck sem GitHub (ADR-0017).
# `pr list` devolve o conteúdo de $FAKE_GH_DIR/prs.json (ou []);
# `pr create` registra os argumentos em $FAKE_GH_DIR/create.log, acrescenta
# o PR ao prs.json com um check em andamento e imprime a URL.
dir="${FAKE_GH_DIR:-${TMPDIR:-/tmp}/fake-gh}"; mkdir -p "$dir"; prs="$dir/prs.json"
[ -f "$prs" ] || echo '[]' > "$prs"
case "$1 $2" in
  "pr list") cat "$prs" ;;
  "pr create")
    shift 2; head=""; title=""
    printf '%q ' "$@" >> "$dir/create.log"; echo >> "$dir/create.log"
    while [ $# -gt 0 ]; do case "$1" in --head) head="$2"; shift 2;; --title) title="$2"; shift 2;; *) shift;; esac; done
    n=$(( $(python3 -c 'import json,sys;print(len(json.load(open(sys.argv[1]))))' "$prs") + 1 ))
    python3 - "$prs" "$n" "$head" "$title" <<'PY'
import json, sys
p, n, head, title = sys.argv[1], int(sys.argv[2]), sys.argv[3], sys.argv[4]
d = json.load(open(p))
d.append({"number": n, "url": f"https://github.com/fake/repo/pull/{n}", "headRefName": head, "title": title,
          "statusCheckRollup": [{"__typename": "CheckRun", "name": "build", "status": "IN_PROGRESS", "conclusion": ""}]})
json.dump(d, open(p, "w"))
PY
    echo "https://github.com/fake/repo/pull/$n" ;;
  *) echo "fake-gh: comando não suportado: $*" >&2; exit 1 ;;
esac
