## ADDED Requirements

### Requirement: Pedido pendente visível
O ccdeck SHALL mostrar, para uma sessão em *Aguardando aprovação*, a ferramenta e o resumo do pedido de permissão vindos dos hooks.

#### Scenario: Pedido de Bash
- **WHEN** o Claude Code dispara `PermissionRequest` com `tool_name = "Bash"` e `tool_input.command = "./mvnw flyway:migrate"`
- **THEN** o painel mostra "Bash" e "./mvnw flyway:migrate" no bloco Pedido pendente

### Requirement: Aprovar e recusar sem entrar
O ccdeck SHALL enviar as teclas configuradas à sessão quando o usuário pressiona `y` ou `x`, apenas se o pedido exibido ainda for o pedido atual.

#### Scenario: Aprovação válida
- **WHEN** a sessão está em *Aguardando aprovação*, o pedido está na tela há mais de 600 ms e o usuário pressiona `y`
- **THEN** o ccdeck envia `approve_keys` à sessão e registra a ação

#### Scenario: Pedido mudou
- **WHEN** um novo evento chegou depois que o pedido foi exibido
- **THEN** `y` não envia nada e a barra de status pede para conferir de novo

#### Scenario: Pedido desconhecido
- **WHEN** a sessão está em aprovação mas o hook não trouxe o pedido
- **THEN** `y` não envia nada e a UI orienta a entrar com Enter

### Requirement: Resposta rápida
O ccdeck SHALL permitir enviar uma linha de texto a uma sessão em *Aguardando você* sem entrar nela.

#### Scenario: Responder
- **WHEN** o usuário pressiona `m`, digita "pode abrir o PR" e confirma
- **THEN** o texto é enviado literalmente à sessão seguido de Enter

### Requirement: Última resposta
O ccdeck SHALL mostrar o último texto do assistente de cada sessão.

#### Scenario: Turno concluído
- **WHEN** chega `Stop` com `last_assistant_message`
- **THEN** o painel mostra essa mensagem no bloco Última resposta
