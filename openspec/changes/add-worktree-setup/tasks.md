# Tasks — add-worktree-setup

## 1. Configuração
- [x] 1.1 `project.rs`: ler e validar `.ccdeck.json` (rejeitar caminhos absolutos e `..` em `copy`)
- [x] 1.2 `ccdeck init-project [dir]`

## 2. Estado
- [x] 2.1 `Session` ganha `base`, `port_slot`, `ports`
- [x] 2.2 Migração v1 → v2

## 3. Criação da sessão
- [x] 3.1 Copiar arquivos para o worktree
- [x] 3.2 Alocar menor slot livre por projeto e calcular portas
- [x] 3.3 Passar `env` + portas no ambiente da sessão tmux
- [x] 3.4 Rodar `setup` numa janela `setup`, gravando o exit code

## 4. UI
- [x] 4.1 Formulário mostra cópias, setup e portas previstas
- [x] 4.2 Painel mostra portas e estado do setup

## 5. Verificação
- [x] 5.1 Testes de slot, portas, validação e migração
- [x] 5.2 Fluxo real com `.ccdeck.json` de exemplo
