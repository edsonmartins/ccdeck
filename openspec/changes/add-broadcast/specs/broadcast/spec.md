## ADDED Requirements

### Requirement: Enviar a várias sessões
O ccdeck SHALL enviar um texto a todas as sessões marcadas que estejam vivas e em *Aguardando você*, depois de confirmação.

#### Scenario: Envio confirmado
- **WHEN** três sessões estão marcadas, o usuário digita "rode os testes" e confirma
- **THEN** as três recebem o texto seguido de Enter e cada envio é registrado como `broadcast`

#### Scenario: Sessão mudou de estado
- **WHEN** uma das sessões marcadas passou a *Trabalhando* antes da confirmação
- **THEN** ela é pulada e a barra de status informa `enviado a 2, pulada 1`

#### Scenario: Elegíveis
- **WHEN** há sessões em *Trabalhando*, *Aguardando aprovação* ou suspensas
- **THEN** elas não aparecem na lista do broadcast
