# Tasks — add-quick-reply

## 1. Hook
- [x] 1.1 `StatusRecord` ganha `pending` (tool, resumo), `last_message`, `transcript_path`
- [x] 1.2 Resumo de `tool_input` por ferramenta (Bash → command; Edit/Write/Read → file_path; WebFetch → url; demais → JSON curto)
- [x] 1.3 Carregar adiante `last_tool` e `last_message` entre eventos
- [x] 1.4 `Stop` grava `last_assistant_message`

## 2. Leitura
- [x] 2.1 `transcript.rs`: último texto do assistente lendo só o fim do JSONL
- [x] 2.2 Fallback para o transcript quando o hook não trouxe a mensagem

## 3. Ações
- [x] 3.1 `tmux::send_keys` (teclas nomeadas) e `send_text` (literal + Enter)
- [x] 3.2 `y`/`x` com travas: estado atual, mesmo `at`, 600 ms de exibição
- [x] 3.3 `m` abre campo de resposta; só em *Aguardando você*
- [x] 3.4 Config `approve_keys` / `deny_keys`

## 4. UI
- [x] 4.1 Bloco "Pedido pendente" com teclas
- [x] 4.2 Bloco "Última resposta"
- [x] 4.3 Rodapé contextual por estado

## 5. Verificação
- [x] 5.1 Testes unitários de resumo, merge de record e leitura de transcript
- [x] 5.2 Fluxo real com `scripts/fake-claude.sh`
