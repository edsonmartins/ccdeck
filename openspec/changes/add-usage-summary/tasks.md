# Tasks — add-usage-summary

## 0. Validação
- [x] 0.1 Conferir num transcript real os campos `message.id`, `message.model`, `message.usage.*` e `timestamp`
- [x] 0.2 Registrar exemplos anonimizados como fixtures de teste

## 1. Coleta
- [x] 1.1 `usage.rs`: leitura incremental por offset, dedupe por `message.id`, agregados por hora e modelo
- [x] 1.2 `state/usage/<sid>.json` com versão do formato
- [x] 1.3 Hook anexa cada evento em `state/events/<sid>.jsonl` (com rotação simples)
- [ ] 1.4 Tempo por estado (aprovação, sua vez, trabalhando) a partir do histórico

## 2. Custo
- [x] 2.1 `prices` no `config.json` com padrão da página oficial (data registrada)
- [x] 2.2 `cost_display`: `api` | `hidden`
- [x] 2.3 `ccdeck doctor` avisa modelos sem preço

## 3. UI
- [ ] 3.1 `View::Summary` e `Tab` alternando Projetos → Atenção → Resumo
- [ ] 3.2 Blocos de números
- [ ] 3.3 Barras de tokens por hora (BarChart) e linha de custo/mensagens (Chart)
- [ ] 3.4 Tabela por sessão, atividade recente, arquivos modificados
- [ ] 3.5 Filtro de período `p` e de projeto `/`
- [ ] 3.6 Linha compacta de uso no painel da sessão

## 4. Verificação
- [ ] 4.1 Testes do parser com fixtures, dedupe e cálculo de custo
- [ ] 4.2 Teste de leitura incremental (arquivo crescendo)
- [ ] 4.3 Fluxo real com transcript gerado pelo `scripts/fake-claude.sh`
