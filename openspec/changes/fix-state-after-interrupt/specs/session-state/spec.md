## ADDED Requirements

### Requirement: Estado depois da recusa
O ccdeck SHALL colocar a sessão em *Aguardando você* depois de enviar a recusa, porque o Claude Code não emite hook nesse caso.

#### Scenario: Recusa com x
- **WHEN** a sessão está em *Aguardando aprovação* e o usuário pressiona `x` com as travas satisfeitas
- **THEN** o ccdeck envia `deny_keys`, grava `status` *Aguardando você* com evento `ccdeck · recusado` e o `m` fica disponível

#### Scenario: Hook chegou no meio
- **WHEN** um novo evento de hook gravou o status entre a leitura e a gravação do ccdeck
- **THEN** o ccdeck não sobrescreve o status

### Requirement: Aviso de sessão sem eventos
O ccdeck SHALL avisar quando uma sessão está em *Trabalhando* sem evento novo há 5 min ou mais, sem mudar o estado.

#### Scenario: Interrompida dentro da sessão
- **WHEN** o usuário interrompeu o turno com `Esc` dentro da sessão e voltou ao ccdeck
- **THEN** depois de 5 min o painel mostra "sem eventos há N min"

### Requirement: Rótulo do modo padrão
O ccdeck SHALL mostrar o modo `default` como "padrão das suas settings", já que não passa `--permission-mode` e vale o `defaultMode` do usuário.
