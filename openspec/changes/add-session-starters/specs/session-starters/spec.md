## ADDED Requirements

### Requirement: Sessão a partir de change do OpenSpec
O ccdeck SHALL oferecer, no formulário de nova sessão, os changes em `openspec/changes/` do projeto e preencher nome e prompt a partir do escolhido.

#### Scenario: Escolher change
- **WHEN** o repositório tem `openspec/changes/add-x/proposal.md` e o usuário escolhe esse change no campo Modelo
- **THEN** o nome vira `add-x`, o branch previsto `ccdeck/add-x` e o prompt manda ler `openspec/changes/add-x/`

#### Scenario: Change fora da base
- **WHEN** o change não está commitado na branch base
- **THEN** o formulário avisa que o worktree não terá os arquivos

#### Scenario: Progresso
- **WHEN** o `tasks.md` tem 3 de 7 tarefas marcadas
- **THEN** o campo mostra `3/7`

### Requirement: Templates do projeto
O ccdeck SHALL oferecer os `templates` do `.ccdeck.json` no campo Modelo.

#### Scenario: Template com modo
- **WHEN** o template `revisar` tem `"mode": "plan"`
- **THEN** escolher o template preenche o prompt e seleciona o modo `plan`
