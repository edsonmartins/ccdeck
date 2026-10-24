## ADDED Requirements

### Requirement: Entrega com confirmação
O ccdeck SHALL commitar, subir o branch e abrir o PR de uma sessão só depois de confirmação explícita, mostrando o que vai acontecer.

#### Scenario: Push
- **WHEN** o usuário escolhe push na entrega
- **THEN** o modal mostra o branch e a URL do `origin`, e só um segundo Enter executa `git push -u origin <branch>`

#### Scenario: PR
- **WHEN** o usuário escolhe abrir PR com o título "Login com SSO"
- **THEN** depois da confirmação o ccdeck roda `gh pr create --base <base> --head <branch> --title "Login com SSO"` e mostra a URL

#### Scenario: Erro visível
- **WHEN** o `gh` falha (sem login, sem remoto)
- **THEN** o modal mostra a mensagem de erro do `gh`

### Requirement: Status do CI
O ccdeck SHALL mostrar o estado dos checks do PR aberto de cada sessão, consultando o `gh` a cada 60 s por projeto.

#### Scenario: Check falhou
- **WHEN** o PR do branch da sessão tem um check com `conclusion` FAILURE
- **THEN** o painel mostra `PR #<n> ✕ 1`
