# Tasks — fix-state-after-interrupt

## 1. Decisão
- [x] 1.1 ADR-0011 e invariante 2 da CONSTITUTION

## 2. Código
- [ ] 2.1 `hook::record_local`: grava status sintético só se o `at` no disco não mudou
- [ ] 2.2 `answer_permission`: recusa bem-sucedida grava *Aguardando você* (`ccdeck · recusado`)
- [ ] 2.3 Aviso "sem eventos há N min" em *Trabalhando* parado há 5 min ou mais
- [ ] 2.4 Rótulo "padrão das suas settings" para o modo `default`

## 3. Verificação
- [ ] 3.1 Testes de `record_local` (grava; não sobrescreve hook novo) e do aviso
- [ ] 3.2 Fluxo real: `x` numa sessão real e `m` em seguida
