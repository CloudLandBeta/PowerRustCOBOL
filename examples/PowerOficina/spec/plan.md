# Plano — Gerenciamento de oficina mecânica (PowerOficina)

- **Spec:** spec.md  **Data:** 2026-11-08  **Tipo de projeto:** Web (SPA) com Backend, tema Elegance claro.

## 1. Arquitetura
- **Navegação:** `main-form` com SideMenu (`webforms/SIDEMENU-1.menu.yaml`), breadcrumb e ContentPane; as telas são formulários `Embedded` carregados no ContentPane (R49, R50).
- **Telas (`webforms/`):** dashboard, atendimentos, diagnóstico, orçamentos, aprovação, fila, pagamentos, relatórios. Cada uma é responsiva (R57).
- **Backend (`backend/src/`, lista em `backend/exports.toml`):** HEALTH, CUSTOMER-SAVE, VEHICLE-SAVE, VEHICLE-FIND, DIAG-COLLECT, CATALOG-LIST, QUOTE-SUGGEST/GET/SAVE/VIEW/SEND/APPROVE/EXPIRE, PAYMENT-CONFIRM, INSTALLMENT-RUN, REFUND-REQUEST, QUEUE-LIST/UPDATE, DASH-SUMMARY, REPORT-LIST, SEED-LOAD. Segredos só aqui (R35, R58).
- **Dados:** arquivos indexados em `indexed/` com dados fictícios de partida (`SEED-LOAD`).
- **Imagens:** `Assets/` (logotipo da navegação), por caminho relativo (R68).

## 2. Mudanças de 2026-11-08
| Área | Decisão | Requisito |
|---|---|---|
| Dashboard | Indicadores como blocos KPI (`KPI-*` painel, `CAP-*` legenda, `VAL-*` valor). | R64 |
| Dashboard | Dois histogramas 3D no `main-form`: `CNV-TOP` e `CNV-DL` (Canvas3D) e `SCN-TOP` (Scene3D de apoio, fora da vista). Procedimentos `PREPARE-3D` (piso, luz e câmera isométrica, uma vez), `DRAW-TOP` (uma barra por item mais vendido, altura = quantidade, legenda sob o gráfico) e `DRAW-DELIVERIES` (por data, uma barra de entregas e uma de retiradas). Cada `CALL` tem o bloco `*>` de comentário. | R63 |
| Paridade RAD × página | Painel do logotipo = `HeaderHeight`; logotipo 270×80 ajustado e centrado; barra lateral fixa na janela; botão abrir/recolher com `BreadcrumbHeight` 41 e `BreadcrumbIconSize` 29 (valores do PowerDemo3). | R65 |
| Grades | `ShowColumnFilters` = 1 em todas; cinco linhas visíveis: nas grades de orçamento `DG-LINES` 240 px, `DG-PARTS` e `DG-ADDONS` 220 px, e os controles abaixo deslocados (cartões 710 px, formulário 1062 px). | R66 |
| Carregamento | Propriedade "Loading effect" nos controles que dependem de dados do Backend; a página mostra listras enquanto há chamada em andamento. | R67 |
| Backend no IDE | Janela do Backend, botão Backend na barra superior (cinza/verde/vermelho; o clique para), Executar reinicia o Backend parado, programas de `backend/src/` não listados são adotados. | R68, R69 |
| IDE | Barra de ferramentas só de ícones (rótulo = dica), barra do desenhista 20% menor; arquivos ocultos (`.DS_Store`) ignorados na construção. | R70 |

## 3. Verificação
- `check` limpo em todos os formulários; `render_form` de cada tela alterada.
- Prévia no navegador: conferir medidas do menu contra o RAD (R65), cinco linhas e filtros nas grades (R66), 3D com dados (R63), listras durante uma chamada (R67).
- Cada item dos critérios AC51–AC57 tem uma conferência manual descrita em `tasks.md`.

## 4. Riscos e pontos em aberto
- **3D no navegador:** os histogramas aparecem em branco na página (canvases montados, sem pixels); a causa só aparecerá com o console `[pr]` da página aberta (T12).
- **Paridade fina:** fontes e espaçamentos de outros controles ainda não medidos em pixels (T10).
- **Duas telas de Dashboard:** existe `dashboard-form.wcfrm` (grades e gráficos planos) e o painel no `main-form`; decidir qual é a Home (T13).
- **Base de conhecimento do sistema:** `LoadingEffect` e legenda de gráfico dependem do modelo semântico baixado no IDE.
