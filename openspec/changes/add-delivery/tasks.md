# Tasks — add-delivery

## 1. Base
- [x] 1.1 `delivery.rs`: parse de `gh pr list`, resumo do CI
- [x] 1.2 `git.rs`: commits à frente, URL do remoto, arquivos não commitados
- [x] 1.3 `gh_command` no `config.json`; `scripts/fake-gh.sh`

## 2. CI
- [x] 2.1 Consulta por projeto a cada 60 s em segundo plano; erro guardado por projeto
- [x] 2.2 PR e CI ao lado do branch no painel

## 3. Entrega
- [x] 3.1 Modal `E` com estado do branch e ações
- [x] 3.2 Confirmação de commit (arquivos), push (destino) e PR (título)
- [x] 3.3 Execução em segundo plano com saída/erro no modal

## 4. Verificação
- [x] 4.1 Testes de parse e CI
- [x] 4.2 Fluxo com remoto local e `fake-gh.sh`
- [ ] 4.3 Fluxo real num repositório GitHub de teste (com autorização)
