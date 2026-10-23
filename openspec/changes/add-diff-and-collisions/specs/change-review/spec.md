## ADDED Requirements

### Requirement: Alterações da sessão
O ccdeck SHALL listar os arquivos alterados por uma sessão em relação ao ponto onde o branch dela saiu da base, incluindo mudanças não commitadas e arquivos novos.

#### Scenario: Commit e arquivo novo
- **WHEN** a sessão commitou uma mudança em `A.java` e criou `B.java` sem commitar
- **THEN** o bloco Alterações lista `A.java` com +/− e `B.java` como novo

### Requirement: Aviso de colisão
O ccdeck SHALL marcar sessões do mesmo projeto que alteraram o mesmo arquivo.

#### Scenario: Mesmo arquivo em duas sessões
- **WHEN** as sessões S1 e S2 alteraram `SecurityConfig.java`
- **THEN** ambas mostram `⚠` na sidebar e o painel de S1 diz que colide com S2 nesse arquivo

### Requirement: Diff completo
O ccdeck SHALL abrir o diff completo da sessão com o comando configurado.

#### Scenario: Abrir diff
- **WHEN** o usuário pressiona `D`
- **THEN** a TUI é suspensa, `diff_command` roda no diretório da sessão e, ao sair, a TUI volta
