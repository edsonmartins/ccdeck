# Tasks — add-delivery

## 1. Base
- [ ] 1.1 `delivery.rs`: parse de `gh pr list`, resumo do CI
- [ ] 1.2 `git.rs`: commits à frente, URL do remoto, arquivos não commitados
- [ ] 1.3 `gh_command` no `config.json`; `scripts/fake-gh.sh`

## 2. CI
- [ ] 2.1 Consulta por projeto a cada 60 s em segundo plano; erro guardado por projeto
- [ ] 2.2 PR e CI ao lado do branch no painel

## 3. Entrega
- [ ] 3.1 Modal `E` com estado do branch e ações
- [ ] 3.2 Confirmação de commit (arquivos), push (destino) e PR (título)
- [ ] 3.3 Execução em segundo plano com saída/erro no modal

## 4. Verificação
- [ ] 4.1 Testes de parse e CI
- [ ] 4.2 Fluxo com remoto local e `fake-gh.sh`
- [ ] 4.3 Fluxo real num repositório GitHub de teste (com autorização)
