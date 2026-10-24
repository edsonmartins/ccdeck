#!/usr/bin/env python3
"""Gera uma fixture anonimizada a partir de um transcript real do Claude Code.

Mantém só a estrutura e os números que o ccdeck usa (ADR-0010): tipo da linha,
timestamp, modelo, stop_reason, usage e o *tipo* de cada bloco de conteúdo.
Texto, caminhos, prompts, ids e saídas de ferramenta são descartados; ids de
mensagem viram `msg_fixture_<n>` preservando quais linhas são da mesma mensagem.

Uso: python3 -I scripts/anonimizar-transcript.py ENTRADA.jsonl SAIDA.jsonl [MAX_LINHAS]
"""
import json
import sys

USAGE_KEYS = ("input_tokens", "output_tokens", "cache_read_input_tokens",
              "cache_creation_input_tokens", "speed", "service_tier")


def main():
    src, dst = sys.argv[1], sys.argv[2]
    limit = int(sys.argv[3]) if len(sys.argv) > 3 else None
    ids = {}
    out = []
    for line in open(src, encoding="utf-8", errors="replace"):
        try:
            d = json.loads(line)
        except ValueError:
            continue
        kind = d.get("type")
        rec = {"type": kind}
        if "timestamp" in d:
            rec["timestamp"] = d["timestamp"]
        if kind == "assistant":
            m = d.get("message") or {}
            mid = ids.setdefault(m.get("id"), f"msg_fixture_{len(ids) + 1}")
            u = m.get("usage") or {}
            usage = {k: u[k] for k in USAGE_KEYS if k in u}
            if isinstance(u.get("cache_creation"), dict):
                usage["cache_creation"] = {
                    k: v for k, v in u["cache_creation"].items()
                    if k in ("ephemeral_5m_input_tokens", "ephemeral_1h_input_tokens")
                }
            rec["isSidechain"] = d.get("isSidechain", False)
            rec["message"] = {
                "id": mid,
                "model": m.get("model"),
                "role": "assistant",
                "stop_reason": m.get("stop_reason"),
                "content": [{"type": c.get("type")} for c in m.get("content") or []
                            if isinstance(c, dict)],
                "usage": usage,
            }
        out.append(json.dumps(rec, separators=(",", ":")))
        if limit and len(out) >= limit:
            break
    with open(dst, "w", encoding="utf-8") as f:
        f.write("\n".join(out) + "\n")


if __name__ == "__main__":
    main()
