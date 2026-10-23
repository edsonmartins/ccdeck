# ADR-0005 — Persistência em JSON local

- Status: Aceito
- Data: 2026-10-23

## Contexto
O volume é pequeno (dezenas de projetos, centenas de sessões) e queremos que o estado seja fácil de inspecionar e consertar à mão.

## Decisão
`state.json` com `schema_version`, gravado de forma atômica a cada mudança. Caminhos via crate `directories` (XDG no Linux, `~/Library/Application Support` no macOS). Status de hooks ficam em arquivos separados para não haver escrita concorrente no `state.json`.

## Alternativas
- **SQLite**: bom para histórico grande; reavaliar quando o histórico de eventos entrar (v0.3).

## Consequências
- Migrações são funções `vN -> vN+1` aplicadas na carga.
