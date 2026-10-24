# ADR-0014 — Modelos de sessão e sessão a partir de change do OpenSpec

- Status: Aceito
- Data: 2026-10-23
- Complementa: ADR-0008

## Contexto
Boa parte das sessões começa do mesmo jeito: "implemente o change X do OpenSpec", "revise o PR", "investigue o teste que quebrou". Hoje o usuário digita o prompt inicial toda vez, e o nome da sessão costuma repetir o id do change.

## Decisão
1. O formulário de nova sessão ganha o campo **Modelo** (`←`/`→`), mostrado só quando há opções. Fontes:
   - **Templates do projeto** em `.ccdeck.json`: `"templates": [{"name": "revisar", "prompt": "Revise as mudanças deste branch…", "mode": "plan"}]` (`mode` opcional).
   - **Changes do OpenSpec** do repositório: cada diretório em `openspec/changes/<id>/` com `proposal.md` (exceto `archive/`). Mostra o título (primeiro `#` do `proposal.md`) e o progresso do `tasks.md` (`- [x]` / `- [ ]`), pendentes primeiro.
2. Escolher um modelo preenche o prompt inicial, o nome da sessão (se vazio, ou se ainda é o nome do modelo anterior) e o modo (se o template definir). Para change do OpenSpec, o nome é o `<id>` (o branch vira `ccdeck/<id>`) e o prompt vem de `openspec_prompt` no `.ccdeck.json`, com `{id}` e `{dir}`; o padrão manda ler `proposal.md`, `tasks.md` e `specs/` e seguir as tarefas na ordem, marcando cada uma.
3. O prompt **referencia** os arquivos, não copia o conteúdo: o Claude Code lê do worktree. Se o change não existe na branch base (`git cat-file -e <base>:openspec/changes/<id>/proposal.md`), o formulário avisa que o worktree não terá os arquivos.

## Alternativas
- **Copiar `proposal.md` e `tasks.md` para o prompt**: prompt enorme numa linha de comando e desatualizado assim que o change muda. Rejeitado.
- **Templates no `config.json` do usuário**: não compartilha com o time; pode vir depois como complemento.

## Consequências
- [inferência] Changes criados e ainda não commitados são o caso comum logo depois de escrever a proposta; o aviso evita uma sessão que não acha os arquivos.
- O `.ccdeck.json` continua opcional: sem templates e sem `openspec/`, o campo não aparece.
