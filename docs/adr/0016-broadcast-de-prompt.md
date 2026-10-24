# ADR-0016 — Broadcast de prompt (`B`)

- Status: Aceito
- Data: 2026-10-23
- Complementa: ADR-0007

## Contexto
Com várias sessões paradas esperando o usuário, a mesma resposta costuma servir para todas: "rode os testes e corrija o que falhar", "faça commit do que está pronto", "atualize o tasks.md". Hoje é um `m` por sessão.

## Decisão
1. `B` abre o broadcast: um campo de uma linha e a lista das sessões **elegíveis**, que são as vivas em *Aguardando você* (de qualquer projeto). Vêm marcadas as do projeto selecionado; `Espaço` marca/desmarca, `*` marca ou desmarca todas.
2. `Enter` mostra a confirmação com os destinos; um segundo `Enter` envia. `Esc` volta ou cancela.
3. No envio, cada destino é conferido de novo no disco (`status/<sid>.json` ainda em *Aguardando você* e a sessão tmux viva), como as travas do ADR-0007. Quem mudou de estado é pulado, e a barra de status diz quantas receberam e quantas foram puladas.
4. O texto é enviado como no `m` (`send-keys -l` + Enter) e cada envio entra no `actions.log` como `broadcast` (aparece na linha do tempo, ADR-0012).

## Alternativas
- **Incluir sessões em *Trabalhando***: o texto cairia no meio de um turno (fila de mensagens do Claude Code), com efeito difícil de prever. Rejeitado.
- **Só sessões do mesmo projeto**: limita o caso "todas as sessões, commitem" entre projetos. Fica como padrão de marcação, não como restrição.

## Consequências
- Um envio errado alcança várias sessões de uma vez; por isso a confirmação com a lista é obrigatória.
