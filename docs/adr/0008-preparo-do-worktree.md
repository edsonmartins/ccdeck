# ADR-0008 — Preparo do worktree por projeto (`.ccdeck.json`)

- Status: Aceito
- Data: 2026-10-23

## Contexto
Um worktree novo não tem arquivos ignorados pelo git (`.env`, configs locais), não tem dependências instaladas e, se duas sessões sobem a aplicação, as portas colidem.

## Decisão
Cada projeto pode ter um `.ccdeck.json` na raiz do repositório, versionado pelo time:

```json
{
  "copy": [".env", "config/application-local.yml"],
  "setup": "npm ci",
  "env": { "SPRING_PROFILES_ACTIVE": "dev" },
  "ports": { "base": 8080, "step": 10, "vars": ["PORT", "SERVER_PORT"] }
}
```

- **copy**: arquivos copiados do repositório principal para o worktree, só se existirem na origem e não existirem no destino. Caminhos absolutos e `..` são rejeitados.
- **setup**: comando rodado no worktree numa janela tmux `setup` da própria sessão, em paralelo ao Claude Code. O código de saída vai para `state/setup/<sid>.exit` e aparece no painel (rodando / ok / falhou).
- **ports**: cada sessão do projeto recebe o menor *slot* livre `n` (1, 2, 3…; o 0 fica para o repositório principal). Cada variável em `vars` vale `base + n*step + i` (i = posição na lista). As variáveis de `env` e de `ports` entram no ambiente da sessão tmux, logo valem para o Claude Code e para os comandos que ele roda.
- `ccdeck init-project` gera um `.ccdeck.json` de exemplo.

## Alternativas
- **Guardar no `state.json`**: não é compartilhável com o time. Rejeitado.
- **Hooks de pós-criação arbitrários**: o `setup` já cobre; mais que isso vira um sistema de plugins.

## Consequências
- O `.ccdeck.json` é do projeto, não do usuário: não conflita com a invariante 3.
- `Session` ganha `base`, `port_slot` e `ports`; `state.json` sobe para `schema_version` 2 (migração preenche padrões).
