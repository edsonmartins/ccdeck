## ADDED Requirements

### Requirement: Sessão ao lado da árvore
O ccdeck SHALL mostrar, no modo integrado, a sessão selecionada ao lado da árvore, sem esconder a árvore.

#### Scenario: Clique numa sessão
- **WHEN** o usuário clica numa sessão viva na barra lateral
- **THEN** o painel da direita passa a mostrar o terminal dessa sessão e a árvore continua visível

#### Scenario: Voltar à árvore
- **WHEN** o foco está no painel da direita e o usuário pressiona `Ctrl-q`
- **THEN** o foco volta para a barra lateral

#### Scenario: Sair
- **WHEN** o usuário pressiona `q` na barra
- **THEN** a sessão emprestada volta para sua janela de origem e as sessões continuam rodando
