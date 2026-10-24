## ADDED Requirements

### Requirement: Contexto usado
O ccdeck SHALL mostrar, por sessão, os tokens de prompt da última mensagem do assistente no transcript principal e a fração da janela do modelo.

#### Scenario: Sessão longa
- **WHEN** a última mensagem teve 452 mil tokens de prompt num modelo com janela de 1M
- **THEN** o painel mostra `contexto 45% (452,0k / 1M)`

#### Scenario: Compactação
- **WHEN** o transcript tem duas linhas `compact_boundary`
- **THEN** o painel mostra `2 compactações` e a linha do tempo lista as duas com antes/depois

### Requirement: Linha do tempo
O ccdeck SHALL mostrar, com `L`, os eventos e as ações da sessão em ordem cronológica inversa, com a duração de cada estado.

#### Scenario: Recusa registrada
- **WHEN** o usuário recusou um pedido com `x`
- **THEN** a linha do tempo mostra a ação `recusar` com o pedido e o evento `ccdeck · recusado`
