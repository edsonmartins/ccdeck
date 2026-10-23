## ADDED Requirements

### Requirement: Contagem de tokens por sessão
O ccdeck SHALL somar, por sessão, hora e modelo, os tokens de entrada, saída, escrita e leitura de cache das mensagens do assistente no transcript, contando cada mensagem uma única vez.

#### Scenario: Mensagem gravada em várias linhas
- **WHEN** o transcript tem três linhas com o mesmo `message.id`
- **THEN** o uso daquela mensagem entra uma vez, com os valores da última linha

#### Scenario: Linha desconhecida
- **WHEN** o transcript tem uma linha que não é JSON válido ou não tem `usage`
- **THEN** a linha é ignorada e o resto é contado normalmente

### Requirement: Custo equivalente em API
O ccdeck SHALL calcular o custo com a tabela `prices` do `config.json` e rotulá-lo como equivalente em API.

#### Scenario: Modelo sem preço
- **WHEN** uma sessão usou um modelo que não está em `prices`
- **THEN** o custo dessa parte aparece como `?` e o `ccdeck doctor` lista o modelo

#### Scenario: Assinatura
- **WHEN** `cost_display` é `hidden`
- **THEN** nenhum valor de custo é exibido

### Requirement: Tempo esperando você
O ccdeck SHALL calcular, por sessão e período, o tempo total em *Aguardando aprovação* e *Aguardando você*.

#### Scenario: Espera somada
- **WHEN** uma sessão ficou 7 min em aprovação e 22 min aguardando resposta no dia
- **THEN** o Resumo de hoje mostra 29 min de espera para ela

### Requirement: Aba Resumo
O ccdeck SHALL oferecer uma aba Resumo com totais, gráficos por hora, tabela por sessão, atividade recente e arquivos modificados, filtrável por período e projeto.

#### Scenario: Trocar período
- **WHEN** o usuário pressiona `p` na aba Resumo
- **THEN** o período alterna entre hoje, 7 dias e 30 dias e todos os blocos são recalculados
