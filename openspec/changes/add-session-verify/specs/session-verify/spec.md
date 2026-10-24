## ADDED Requirements

### Requirement: Rodar a verificação
O ccdeck SHALL rodar o comando `verify` do `.ccdeck.json` no diretório da sessão quando o usuário pressiona `t`.

#### Scenario: Testes passam
- **WHEN** o projeto tem `"verify": "npm test"` e o usuário pressiona `t` numa sessão viva
- **THEN** o comando roda na janela `verify` da sessão e, ao terminar com 0, o painel mostra `ok` com a duração e a barra de status avisa

#### Scenario: Testes falham
- **WHEN** o comando termina com código 1
- **THEN** o painel mostra `falhou (código 1)` e `T` abre o log

#### Scenario: Sem verify
- **WHEN** o `.ccdeck.json` não tem `verify`
- **THEN** `t` não roda nada e a barra de status explica como configurar

#### Scenario: Já rodando
- **WHEN** a verificação da sessão ainda está rodando
- **THEN** `t` não dispara outra
