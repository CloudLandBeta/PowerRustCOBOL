<!--
SPDX-License-Identifier: Apache-2.0
Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors

Licensed under the Apache License, Version 2.0.
See the LICENSE file in the project root for full license information.
-->

# PowerAnalytics

Painel de análise dos **fundos de investimento** da planilha ANBIMA
`Assets/xlsx/FUNDOS-175-PERIODICOS-PUBLICO.xlsx` (43.750 fundos, referência 02/10/2026).
Uma única janela com duas páginas, escolhidas nos botões do topo:

| Botão | Conteúdo |
|---|---|
| **Dashboard** | KPI principal com sparkline, Fundos criados por ano (barras, 3 séries), Estrutura (**RadarChart**, 3 séries × até 10 eixos), Maiores fundos, PL por categoria (donut, 10 fatias), Fundos por nível 2 (donut, 10 fatias), Evolução por ano de início (linhas, 3 séries) e Faixas de PL. |
| **Dados** | `DataGrid` com os 16 campos da planilha (300 linhas por página, ordenação por PL, cotistas, início ou nome) e, ao lado, uma faixa com 5 painéis de KPI calculados sobre o filtro ativo. |

## Filtros (valem para as duas páginas)

- **Nome comercial** e **Categoria** — campos de busca com lista suspensa. Ao digitar, a lista mostra os nomes **únicos** que contêm o texto (ignora acentos e maiúsculas: `itaú` acha `ITAÚ`), até 150 por vez, com a contagem total no rodapé. A primeira linha é **Todos**; as demais têm caixa de seleção.
  - Sem nada marcado, vale o texto digitado (todos os fundos que o contêm).
  - Marcando itens, **só eles** entram no filtro, e o rótulo do campo passa a dizer quantos estão marcados (`Nome comercial · 2 marcado(s)`).
  - **Todos** marca tudo e usa todos os resultados da busca, inclusive os que a lista não exibe. Desmarcar um item sai do modo Todos; desmarcar Todos limpa a seleção.
  - **×** limpa o texto e volta a listar todos os nomes únicos; **▾** abre/fecha a lista; **Limpar seleção** desmarca tudo; **Aplicar** (ou `Enter`) fecha e aplica. `Esc` ou um clique fora da lista também a fecham.
  - **Categoria** é o *Nível 1* da planilha (Multimercados, FIDC, Renda Fixa, Previdência, Ações…).
  - As listas e o combo de ordenação usam **cores de alto contraste**: linha ativa em azul profundo com texto branco (8,4:1), itens marcados em azul-claro com texto azul-marinho (11:1) e caixas marcadas em azul profundo com ✓ branco.
- **Início de atividade (de / até)** — faixa de datas.
- **PL em R$ milhões (de / até)** — faixa de valores.
- **Níveis 2 e 3** — duas listas com caixas de seleção; marque quantos itens quiser. Dentro de um nível os itens se somam (OU); entre os filtros, restringem (E).
- **Aplicar** / **Limpar** (zera todos os filtros, inclusive as seleções).

Os gráficos de barras e linhas usam as **3 maiores categorias** do filtro como séries; donuts e radar usam até **10** categorias.

## De onde vêm os dados

O runtime lê um arquivo de texto com uma instrução SQL por linha (`data/fundos.sql`) e o carrega num
SQLite em memória (`:memory:`) ao abrir a janela — cerca de 0,25 s em build release. Todos os números
saem de consultas SQL (`WHERE`, `GROUP BY`) montadas a partir dos filtros.

`data/fundos.sql` foi gerado **uma vez** a partir da planilha:

- tabela `fundos` com as 16 colunas originais, mais `nome_key` (nome em maiúsculas e sem acentos, para a busca) e `ano`;
- datas do Excel (número serial) convertidas para `AAAA-MM-DD`; células vazias viram `NULL`;
- índices em `nivel1`, `nivel2`, `nivel3`, `ano`, `inicio`, `pl` e `cotistas`.

Para atualizar os dados com uma planilha nova, regere esse arquivo com o mesmo esquema.

> **Por que não abrir um `.db` direto?** Hoje `SqlDatabase::Open` e `EXEC SQL CONNECT` resolvem um
> caminho SQLite relativo a partir do diretório de trabalho do processo, e não da pasta do projeto /
> do executável (que é o que `ASSIGN TO` faz). Um arquivo lido por `ASSIGN` funciona em qualquer lugar.

## Visual e comportamento

- Janela **50 % transparente** (tema Spatial; o sistema desfoca o fundo no macOS e no Windows) sobre um fundo azul-acinzentado **claro**, como na imagem de referência; cada painel é um `GroupBox` branco com `Transparency` 30 — o `GroupBox` esmaece só a moldura, de modo que texto e gráficos continuam nítidos — e o cartão principal é azul-marinho profundo.
- Cada painel projeta uma **sombra suave** (`ShadowEnabled`, `ShadowOpacity` 4, `ShadowDistance` 10, `ShadowBlurStrength` 14) só do lado de fora: num painel translúcido a sombra não aparece através do fundo (correção do motor na 1.90.8 — é preciso um IDE/`rcrun` dessa versão ou mais nova).
- No topo, à direita do painel superior e separado dele, o botão redondo vermelho **×** (`BTN-CLOSE`, com a mesma altura do painel, 64 px) fecha a janela (`INVOKE me::Close()`). Em tela de celular o logotipo some para o botão caber ao lado de Dashboard e Dados.
- Painéis entram com a animação **ZoomOut / Elastic** de **1200 ms**, cada um começando **600 ms** depois do anterior, para chegarem em ordem visível (animação `intro` no carregamento e `replay` ao trocar de página, via `PlayAnimation`).
- Os **gráficos** ficam ocultos até o **último** painel terminar de entrar; só então aparecem **todos ao mesmo tempo** e suas barras, linhas, fatias e polígonos **crescem de zero até os valores** em **1500 ms** (`AnimateOnLoad`). Ao **aplicar filtros**, os gráficos que já estão na tela passam dos valores antigos para os novos na mesma duração (`AnimateValues`). Um `Timer` (`TMR-STAGE`) mostra os gráficos na hora certa. Se a carga dos dados demorar mais que as animações, os gráficos aparecem de uma vez, já com os dados.
- As listas suspensas (busca de Nome/Categoria e Níveis 2 e 3) são cartões opacos (`SM-DROP`, `LV-DROP`) que flutuam sobre a página; uma camada transparente (`SCRIM`) abaixo delas fecha a lista num clique fora. No **designer** esses cartões aparecem sobre o painel porque ele mostra os controles ocultos; em execução ficam escondidos até serem abertos.
- **Responsivo** (`responsive="true"`): ≥ 1024 px, grade de 4 colunas; 600–1023 px, cartões em fluxo (dois por linha onde cabem); < 600 px (celular), uma coluna com rolagem vertical e o donut de PL por categoria omitido. A janela pode ser reduzida até ~390 px de largura.

## Requisitos

- O gráfico de estrutura usa o controle **`RadarChart`** (novo) com uma escala comum de 0 a 100 escrita no primeiro eixo, contorno opaco e preenchimento translúcido, como recomendam as diretrizes de gráficos de radar (data-to-viz): são só três séries, indexadas ao líder de cada uma. Use um `rcrun`/IDE construído com esse controle.
- Para abrir/rodar: abra a pasta no PowerRustCOBAL AI e execute o formulário `forms/main-form.cfrm` (formulário principal).
