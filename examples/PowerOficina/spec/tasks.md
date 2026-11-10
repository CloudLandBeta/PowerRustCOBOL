# Tarefas — PowerOficina

Marcação: `[x]` feita e verificada, `[ ]` aberta. Cada tarefa cita o requisito.

## Telas e Backend (base)
- [x] T1 Criar o projeto Web com Backend, tema Elegance claro e navegação lateral (R49, R50, R59).
- [x] T2 Formulários: dashboard, atendimentos, diagnóstico, orçamentos, aprovação, fila, pagamentos, relatórios (R1–R33).
- [x] T3 Programas do Backend e `exports.toml` (R35, R58).
- [ ] T4 Aceitação AC1–AC50 conferida uma a uma (spec §5).

## Revisão de 2026-11-08
- [x] T5 Blocos KPI no Dashboard (R64).
- [x] T6 Histogramas 3D no `main-form`: `CNV-TOP`, `CNV-DL`, `SCN-TOP`; procedimentos `PREPARE-3D`, `DRAW-TOP`, `DRAW-DELIVERIES` com comentário `*>` em cada `CALL` (R63).
- [x] T7 Logotipo em `Assets/` com caminho relativo; Backend adotado de `backend/src/` (R68).
- [x] T8 Botão abrir/recolher com `BreadcrumbHeight` 41 e `BreadcrumbIconSize` 29 em `main-form.wcfrm` (R65).
- [x] T9 Painel do logotipo, barra lateral fixa e medidas do menu no RAD e na página (R65), no PowerRustCOBOL AI 2.0.315 e 2.0.320.
- [ ] T10 Medir em pixels fontes e espaçamento vertical dos itens do menu e dos demais controles, RAD × página (AC53).
- [x] T11 "Loading effect" (R67), no PowerRustCOBOL AI 2.0.320; falta marcar os controles do projeto que dependem de dados do Backend (AC55).
- [ ] T12 3D em branco na página: ler o console `[pr]` da prévia, achar a causa e corrigir (AC51).
- [ ] T13 Decidir a Home (Dashboard do `main-form` × `dashboard-form`) e remover a duplicata com confirmação do operador (código do desenvolvedor não se apaga sem confirmação).
- [x] T14 Grades de orçamento: `ShowColumnFilters` e alturas para cinco linhas (R66); a mesma propriedade nas demais grades.
- [ ] T15 Conferir `check`, `render_form` e a prévia de `orcamentos-form` com a nova disposição; ajustar `fila-form` (230 px), `dashboard-form` (196 px) e `pagamentos-form` (250 px) se a linha de filtro deixar menos de cinco linhas (AC54).
- [x] T16 Botão Backend, janela do Backend, reexecução após parar e arquivos ocultos ignorados (R69, R70), PowerRustCOBOL AI 2.0.321 a 2.0.326.
- [ ] T17 Atualizar a base de conhecimento do sistema (`LoadingEffect`, legenda de gráfico) quando o modelo semântico estiver baixado.
