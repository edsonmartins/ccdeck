## ADDED Requirements

### Requirement: Abrir janela auxiliar
O ccdeck SHALL criar, sob demanda, janelas tmux na sessão com o comando configurado, no diretório e no ambiente da sessão, e entrar nelas.

#### Scenario: Servidor nas portas da sessão
- **WHEN** o projeto tem `"windows": {"servidor": "npm run dev"}` e a sessão tem `PORT=8090`, e o usuário escolhe `servidor` em `w`
- **THEN** a janela `servidor` é criada no worktree com `PORT=8090` e o ccdeck entra nela

#### Scenario: Janela já existe
- **WHEN** a janela já existe
- **THEN** `Enter` só entra nela, sem criar outra

### Requirement: Fechar janela auxiliar
O ccdeck SHALL fechar uma janela auxiliar só depois de confirmação, e nunca a janela `claude`.

#### Scenario: Confirmação
- **WHEN** o usuário pressiona `K` uma vez sobre `servidor`
- **THEN** nada é fechado e a lista pede `K` de novo
