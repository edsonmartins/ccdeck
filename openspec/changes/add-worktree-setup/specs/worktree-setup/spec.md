## ADDED Requirements

### Requirement: Cópia de arquivos locais
O ccdeck SHALL copiar para o worktree novo os arquivos listados em `copy` que existam no repositório principal e não existam no worktree.

#### Scenario: .env copiado
- **WHEN** o projeto tem `"copy": [".env"]` e o repositório tem `.env`
- **THEN** o worktree da nova sessão tem uma cópia de `.env`

#### Scenario: Caminho inseguro
- **WHEN** `copy` contém `../segredo` ou `/etc/passwd`
- **THEN** o item é ignorado e a barra de status avisa

### Requirement: Portas por sessão
O ccdeck SHALL dar a cada sessão de um projeto um slot exclusivo e exportar as portas derivadas no ambiente da sessão.

#### Scenario: Duas sessões
- **WHEN** o projeto tem `"ports": {"base": 8080, "step": 10, "vars": ["PORT"]}` e já existe uma sessão no slot 1
- **THEN** a nova sessão recebe o slot 2 e `PORT=8100`

### Requirement: Setup em paralelo
O ccdeck SHALL rodar o comando `setup` no worktree numa janela tmux separada e mostrar seu resultado.

#### Scenario: Setup falha
- **WHEN** o `setup` termina com código diferente de zero
- **THEN** o painel mostra "setup: falhou (código N)"
