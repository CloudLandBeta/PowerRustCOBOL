# Spec 110 — Caption do TextBox (linha 2.x)

## Context
O TextBox ganha um caption opcional (Top/Bottom/Left/Right) cuja posição é sempre calculada a partir da caixa, com fonte/cor herdadas por atributo e override. Pedido do operador; linha 2.x confirmada.

## Estado atual
- Worktree `.claude/worktrees/features-2x-caption`, branch `features-2x-caption` cortada de `2.x.x` (2b9e0a3a). Versão base 2.0.110.
- `crates/cobolt-forms/src/caption.rs` já escrito (tipos puros, `caption_rect`, `outer_bounds`, `FontOverride::resolve`, `parse_mnemonic`, testes da tabela da seção 9). **Ainda não registrado em `lib.rs` e não compilado**: o `sed` falhou (BSD sed); registrar com Edit (`pub mod caption;` antes de `pub mod chart;`).
- Achado de modelo: `Control.properties` é `IndexMap<String, PropValue>` e só contém chaves presentes. Logo "definido" = chave presente, "herdado" = ausente; basta NÃO semear `Caption*` em `Control::new` e o round-trip do `.cfrm` preserva a herança sem trabalho extra.
- `cobolt-semantic` não tem tabela de propriedades por controle; a mudança cruza crates (ver fases).

## Decisões adotadas (defaults da seção 10)
Herança por atributo; Top+Left, gap 4; Middle geométrico; desabilitado sobrepõe a cor; sem `CaptionWidth`; gap em unidade lógica (`Unit`). Sintaxe COBOL do projeto: `TXT::CaptionText`, `TXT::ResetCaption(...)`, `TXT::CaptionIsSet(...)`.

## Fases
1. Núcleo puro: registrar módulo, `cargo test -p cobolt-forms --lib caption::`.
2. Modelo/arquivo: propriedades `Caption*` em `model.rs` (`property_names_for`, set/get_property), leitura/gravação em `xml.rs`; teste de round-trip herdado×definido e form antigo idêntico.
3. Runtime layout/desenho nos DOIS caminhos (`paint.rs` e `render.rs`), medição via `textmetrics.rs` (mesma função no designer); Visible/Enabled.
4. Interação: hit-test do caption → foco, mnemônico Alt+tecla, nome acessível.
5. COBOL: `ResetCaption`, `CaptionIsSet`; tabelas de doc em `cobolt-compiler`, `cobolt-project-tools/src/reference.rs`, `cobolt-codegen` se houver tabela estática; regenerar `assets/knowledge/chunked.data`.
6. Designer (`cobolt-ide/src/panels/designer.rs`): grupo Caption, herdado cinza/definido negrito, seleção/arraste, snap em inner bounds, avisos, undo; strings `Tr` nas 6 línguas.
7. Integração: subform/outlet; paridade nos hosts (rcrun, `run_form_app`, `cobolt-wasm`, `cobolt-web-player`); testes com bloco de resultado quantificado (regra #7).

## Regras do repositório a cumprir
- Trabalhar só no worktree; não tocar a árvore compartilhada (tem trabalho não commitado da spec 109).
- Em 2.x.x NÃO tocar `README.md` nem `docs/` (Developer's Guide espera o merge em main).
- Bump de `z` em `crates/cobolt-ide/src/version.rs` + entrada no topo de `CHANGELOG.md` (ler só as 60 primeiras linhas). **Colisão:** a árvore compartilhada já tem 2.0.111 não commitada (spec 109 S5); decidir o número no momento do commit.
- Sem push 09–18 seg–sex (São Paulo). Merge em 2.x.x só se pedido.
- Salvar a spec em `specs/110-textbox-caption/spec.md` (criar a pasta; ainda não tem arquivo).
- Credenciais: varrer o diff staged antes de commitar.

## Verificação
- `cargo test -p cobolt-forms --lib caption::` e `cargo test -p cobolt-forms --features render --no-fail-fast` (ler cada `test result:`).
- Teste de round-trip `.cfrm` (herdado×definido) e de form antigo.
- Checklist da seção 9 item a item; `cobolt-ide` testes com `--bin`.
- Guarda de paridade `engine_reference_form_parity_static_vs_faces`.
