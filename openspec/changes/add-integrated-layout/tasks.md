# Tasks — add-integrated-layout

## 0. Validação
- [x] 0.1 Mecanismo de swap-pane, marcas de painel e Ctrl-q condicional no tmux 3.8

## 1. Protótipo (`ccdeck ui`)
- [x] 1.1 Marcas `@ccdeck_session` e alvos por painel (send-keys, capture, sessões vivas)
- [x] 1.2 Sessão `ccdeck-ui`: criar/reaproveitar, painel de espera, attach
- [x] 1.3 Mostrar a selecionada (swap), devolver ao sair/remover, recuperar se o painel morrer
- [x] 1.4 Barra lateral: desenho compacto, mouse, zoom em modais
- [x] 1.5 Testes e fluxo real

## 2. Depois da aprovação
- [x] 2.1 ADR-0018 aceito, CONSTITUTION ajustada, `ccdeck` abre no modo integrado
- [x] 2.3 Foco visível (barra e borda do tmux), abas e botões `+ Sessão`/`+ Projeto`/`? Ajuda` clicáveis, legenda movida para a ajuda
- [x] 2.4 Divisória redimensionável: arraste ou `<`/`>`, mínimo de 30 colunas, largura guardada em `state/sidebar_cols`
- [ ] 2.2 Miniaturas das outras janelas da sessão (shell, servidor) embaixo
