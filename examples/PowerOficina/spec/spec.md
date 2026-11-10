# Spec — Gerenciamento de oficina mecânica (aplicação de exemplo)

- **Status:** draft
- **Folder:** ~/Documents/PowerOficina/spec/
- **Author:** Claude (a pedido do operador)   **Date:** 2026-11-07

## 1. Overview
Uma aplicação de gerenciamento para oficina mecânica, escrita em RustCOBOL
como projeto de exemplo do PowerRustCOBOL AI. Ela cobre o ciclo completo de
uma solicitação de reparo: cadastro do cliente e do veículo, diagnóstico por
conexão eletrônica com o veículo, definição de peças e esforço, serviços
adicionais, orçamento enviado por email e WhatsApp, aprovação do cliente e dos
prazos, fila de execução, pagamento por cartão de crédito via gateway, recibo
e aviso da data de retirada.

## 2. Goals / Non-goals
- **Goals:**
  - Um único fluxo, do atendimento à retirada, sem retrabalho de digitação.
  - Orçamento reproduzível: cada valor pode ser rastreado até peças, horas e adicionais.
  - Cliente informado por email e WhatsApp em cada marco (orçamento, recibo, retirada).
  - Dados do veículo vindos do diagnóstico, não digitados.
- **Non-goals:**
  - Estoque, compras e fornecedores de peças (apenas catálogo com preço).
  - Contabilidade, nota fiscal e folha de pagamento.
  - Diagnóstico automático por IA (o consultor decide; a aplicação calcula e registra).
  - Aplicativo para o cliente final (o cliente interage por email/WhatsApp e link de aprovação).

## 3. User stories
- Como **consultor**, quero cadastrar cliente e veículo uma vez e reutilizá-los, para não redigitar a cada visita.
- Como **consultor**, quero conectar o veículo ao computador e ver os dados coletados na ficha de entrada, para basear o orçamento em medições reais.
- Como **consultor**, quero que a aplicação proponha a lista de peças e o esforço da análise, para ganhar tempo e padronizar preços.
- Como **consultor**, quero oferecer serviços adicionais (limpeza de carpetes, filtros, óleo, lavagem geral) com um clique, para aumentar o valor do atendimento.
- Como **cliente**, quero receber o orçamento por email e WhatsApp e aprová-lo com os prazos, para decidir sem ir à oficina.
- Como **gerente da oficina**, quero uma fila de execução ordenada, para saber o que está pendente e o que está pronto.
- Como **cliente**, quero pagar com cartão e receber o recibo e a data de retirada, para encerrar a contratação com segurança.

## 4. Requirements (EARS)

### Cadastro
- **R1 (ubiquitous):** The system shall registrar cliente com nome, documento (CPF/CNPJ), endereço, telefone, WhatsApp e email.
- **R2 (ubiquitous):** The system shall registrar veículo com placa, chassi, marca, modelo, ano, cor e quilometragem, vinculado a um cliente.
- **R3 (event):** When o consultor informa uma placa já cadastrada, the system shall carregar o veículo e o cliente existentes em vez de criar duplicatas.
- **R4 (ubiquitous):** The system shall validar CPF/CNPJ, email e formato de telefone antes de gravar.

### Diagnóstico
- **R5 (event):** When o consultor conecta o veículo e inicia a coleta, the system shall registrar a data/hora de entrada, o modelo do veículo e os parâmetros lidos pelos sensores (por exemplo códigos de falha, temperatura do motor, rotação, tensão da bateria, pressão de óleo).
- **R5a (constraint):** Nesta versão a coleta é simulada: os dados vêm de um gerador/arquivo de teste por trás de uma interface de coleta única, de modo que um leitor real (programa externo ou adaptador OBD-II) possa substituí-lo sem mudar o resto da aplicação.
- **R6 (state):** While a coleta está em andamento, the system shall mostrar o progresso e os valores já recebidos.
- **R7 (unwanted):** If a conexão com o veículo falhar ou for interrompida, the system shall preservar os dados já recebidos, informar o consultor e permitir retomar ou digitar os parâmetros manualmente, marcando-os como manuais.
- **R8 (ubiquitous):** The system shall associar cada coleta a uma única ordem de análise e manter o histórico de coletas por veículo.

### Análise: peças e esforço
- **R9 (event):** When a coleta é concluída, the system shall sugerir a relação de peças necessárias a partir dos códigos de falha e parâmetros, usando um catálogo de peças e uma tabela de regras de diagnóstico mantidos na aplicação.
- **R10 (ubiquitous):** The system shall permitir ao consultor adicionar, remover ou alterar quantidade de qualquer peça sugerida, registrando que a alteração foi manual.
- **R11 (event):** When a lista de peças é definida, the system shall calcular o esforço de execução em horas de mão de obra por serviço, a partir de uma tabela de tempos padrão, e o custo correspondente pela tarifa/hora vigente.
- **R12 (ubiquitous):** The system shall permitir ao consultor ajustar as horas de um serviço, com justificativa registrada.

### Serviços adicionais
- **R13 (ubiquitous):** The system shall oferecer um catálogo de serviços adicionais, inicialmente: limpeza de carpetes, troca de filtros, troca de óleo e lavagem geral, cada um com preço e esforço.
- **R14 (event):** When o consultor marca ou desmarca um adicional, the system shall atualizar o total do orçamento imediatamente.
- **R15 (optional):** Where o administrador cadastra novos adicionais, the system shall oferecê-los sem alteração de código.

### Orçamento
- **R16 (event):** When o consultor conclui a análise, the system shall gerar o orçamento com cliente, veículo, parâmetros relevantes, peças, mão de obra, adicionais, total, prazo de execução e validade do orçamento. O prazo é sugerido pela fila e pela capacidade diária de horas da oficina, e o consultor pode ajustá-lo; o orçamento mostra o prazo final acordado.
- **R17 (ubiquitous):** The system shall numerar os orçamentos de forma sequencial e imutável; alterar um orçamento enviado gera uma nova revisão, mantendo as anteriores.
- **R18 (event):** When o consultor confirma o envio, the system shall enviar o orçamento por email e por WhatsApp ao contato do cliente e registrar o resultado de cada envio (enviado, falhou, hora). Nesta versão o envio é simulado atrás de uma interface única de notificação: a mensagem é gravada como enviada e nada sai da aplicação; provedores reais de email e WhatsApp entram depois sem mudar o resto.
- **R19 (unwanted):** If um dos canais falhar, the system shall tentar novamente de forma limitada, informar o consultor e não impedir o outro canal.
- **R20 (event):** When o prazo de validade do orçamento vence sem resposta, the system shall marcá-lo como expirado.

### Aprovação
- **R21 (event):** When o cliente aprova o orçamento e o prazo de execução, the system shall registrar a aprovação com data/hora e canal (link recebido por email/WhatsApp, ou registro feito pelo consultor a partir de resposta por telefone, WhatsApp ou balcão), e mudar o estado para "aprovado, aguardando pagamento".
- **R22 (event):** When o cliente recusa ou pede alteração, the system shall registrar o motivo e devolver o orçamento ao consultor para nova revisão ou encerramento.
- **R23 (ubiquitous):** The system shall impedir a execução de qualquer serviço de um orçamento que não esteja aprovado.

### Pagamento
- **R24 (event):** When o orçamento é aprovado, the system shall criar a cobrança no gateway de pagamento para cartão de crédito, pelo valor total aprovado.
- **R25 (ubiquitous):** The system shall não armazenar número completo do cartão nem código de segurança; apenas o identificador da transação, os quatro últimos dígitos e a bandeira.
- **R26 (event):** When o gateway confirma o pagamento, the system shall registrar a transação e colocar os serviços na fila de execução.
- **R27 (unwanted):** If o gateway recusar ou não responder, the system shall manter o orçamento aprovado sem pagamento, informar o motivo e permitir nova tentativa; a cobrança nunca deve ser duplicada para o mesmo orçamento.

### Fila de execução
- **R44 (ubiquitous):** The system shall permitir ao cliente pagar em 1 a 12 parcelas no cartão de crédito, sem juros, vendo antes da confirmação o número de parcelas, o valor de cada uma, as datas de vencimento e o total (que é sempre igual ao valor do orçamento). A primeira parcela é cobrada no ato da contratação e cada parcela seguinte 30 dias após a anterior (30, 60, 90 dias…).
- **R44a (event):** When o gateway confirma a primeira parcela, the system shall considerar a contratação efetivada e colocar os serviços na fila de execução, mesmo que existam parcelas futuras.
- **R44b (event):** When chega a data de vencimento de uma parcela futura, the system shall receber do gateway o resultado da cobrança dessa parcela, feita pela cobrança recorrente do próprio gateway sobre a autorização dada pelo cliente na contratação, sem que o cliente digite o cartão de novo, e registrar o resultado. A aplicação não tem agendador próprio.
- **R44c (unwanted):** If a cobrança de uma parcela futura for recusada ou o gateway não responder, the system shall registrar a falha, avisar o cliente por email e WhatsApp e o consultor no Dashboard, tentar novamente de forma limitada e jamais cobrar a mesma parcela duas vezes.
- **R44d (constraint):** The system shall obter do cliente, antes da primeira cobrança, o consentimento expresso para as cobranças futuras das parcelas, registrando data, hora, número de parcelas e valores; sem esse consentimento só é possível pagar à vista (1 parcela).
- **R45 (ubiquitous):** The system shall registrar, por pagamento, o número de parcelas, o valor de cada parcela, a moeda e o identificador da transação, sem guardar dados do cartão (R25).
- **R46 (event):** When o cliente ou o consultor (qualquer um dos dois) pede estorno de um pagamento, the system shall registrar quem pediu e aceitar o estorno somente para os serviços cujo estado seja "na fila" (ainda não iniciados), pelo valor desses serviços, e criar o estorno no gateway; em pagamento parcelado, o sistema cancela primeiro as parcelas futuras ainda não cobradas desse valor e devolve, pelo gateway, o que já foi pago dele.
- **R47 (unwanted):** If algum serviço do pedido estiver "em execução" ou "concluído", the system shall recusar o estorno desse serviço, mostrar o motivo e manter o pagamento correspondente.
- **R48 (event):** When um estorno é confirmado pelo gateway, the system shall retirar da fila os serviços estornados, registrar o estorno (valor, moeda, hora, motivo) e avisar o cliente por email e WhatsApp.
- **R28 (state):** While há serviços pagos aguardando, the system shall exibir a fila de execução ordenada por prioridade e data de aprovação.
- **R29 (event):** When um serviço muda de estado (na fila, em execução, concluído), the system shall registrar o horário e o responsável.
- **R30 (event):** When todos os serviços de uma ordem estão concluídos, the system shall marcar o veículo como pronto para retirada.

### Recibo e retirada
- **R31 (event):** When o pagamento é confirmado, the system shall emitir o recibo da contratação com número, itens, valor, forma de pagamento e data, e enviá-lo por email e WhatsApp.
- **R32 (event):** When o prazo de execução é definido ou alterado, the system shall informar ao cliente, por email e WhatsApp, a data em que deve retornar para retirar o veículo.
- **R33 (event):** When o veículo fica pronto, the system shall avisar o cliente pelos mesmos canais.

### Interface: SPA e dashboard
Referência visual (apenas inspiração, não cópia): `spec/inspiration/dashboard-ui.webp`. Dela se tomam a estrutura, a hierarquia e a paleta aproximada: coluna de navegação lateral em azul royal saturado, com avatar do operador no topo e "Ajuda" e "Sair" no rodapé; fundo cinza-azulado muito claro; cartões brancos de cantos arredondados; barra superior com busca, notificações e avatar; azul como cor principal, com verde, rosa e roxo como cores de apoio e de variação positiva/negativa.

- **R49 (ubiquitous):** The system shall ser uma aplicação Web de página única (SPA), executada no navegador: uma só página, com a navegação lateral sempre visível e uma área de conteúdo onde cada tela é carregada, sem abrir janelas novas nem recarregar a página inteira ao navegar, mantendo o contexto (operador, moeda, filtros) entre telas.
- **R50 (ubiquitous):** The system shall oferecer a navegação lateral com, no mínimo: Dashboard, Atendimentos (cadastro e entrada), Diagnóstico, Orçamentos, Fila de execução, Pagamentos, Relatórios e Configurações (tabelas e catálogos), mais Ajuda e Sair no rodapé.
- **R51 (ubiquitous):** The system shall mostrar uma barra superior com busca global (cliente, placa, número de orçamento), notificações (aprovações recebidas, pagamentos, falhas de envio) e o nome do operador.
- **R52 (ubiquitous):** The system shall abrir no Dashboard, com cartões de indicadores de atendimento: veículos em atendimento, orçamentos aguardando aprovação, serviços em execução, veículos prontos para retirada, taxa de aprovação de orçamentos, valor total aprovado e ticket médio.
- **R53 (ubiquitous):** The system shall mostrar no Dashboard, além dos cartões: um gráfico de orçamentos enviados e aprovados por mês; a fila de execução resumida (ordem, veículo, serviço, estado, prazo); uma lista ou grade de datas com os prazos de execução e as retiradas previstas (a plataforma não tem controle de calendário); uma tabela dos serviços e peças mais vendidos; a distribuição dos pagamentos (à vista, parcelado, estornado) e dos canais de aprovação (link, manual).
- **R54 (constraint):** The system shall mostrar no Dashboard apenas indicadores que tenham dados reais na aplicação; itens da referência sem origem nos dados (por exemplo avaliação de clientes, dispositivos utilizados) não entram.
- **R55 (constraint):** The system shall aplicar R41 e R42 ao Dashboard: todo indicador de valor mostra uma só moeda por vez, escolhida por um seletor BRL/EUR visível, e nunca soma moedas.
- **R56 (state):** While o Dashboard está aberto, the system shall atualizar os indicadores e a fila quando a situação de um orçamento, pagamento ou serviço mudar, sem exigir que o operador recarregue; a atualização é por consulta periódica ao Backend, porque a página ainda não recebe eventos do servidor.
- **R58 (constraint):** The system shall guardar dados, chaves de gateway, email e WhatsApp e executar cobranças agendadas e envios apenas no Backend; a página no navegador não guarda segredos (R35).
- **R59 (constraint):** The system shall usar o tema Elegance em versão clara e a paleta da referência (R57); o tema é decisão do operador e substitui, para este projeto, o padrão Spatial. Como o Elegance só existe escuro, o visual claro vem de folhas de estilo do próprio projeto. Todo texto tem alto contraste (razão mínima 4,5:1, nível WCAG AA): texto escuro sobre cartões brancos e texto branco sobre o azul da coluna lateral.
- **R57 (ubiquitous):** The system shall manter a mesma paleta, os mesmos cartões e a mesma hierarquia visual em todas as telas, e trocar a disposição conforme o tamanho da janela (colunas que se empilham, navegação lateral que se recolhe em telas estreitas).
- **R60 (constraint):** The system shall mostrar toda informação tabular (mais de um valor por linha: fila, relatórios, parcelas, linhas do orçamento, catálogo, itens mais vendidos, datas) em grade de dados com cabeçalho de colunas, ordenação e colunas redimensionáveis; lista de uma só coluna só para escolhas simples, nunca texto montado em colunas.
- **R61 (state):** While um gráfico do Dashboard é mostrado ou recebe dados novos, the system shall animá-lo (as marcas crescem na primeira carga e se movem quando os dados mudam) e mostrar o valor sob o ponteiro (dica de valor).
- **R62 (event):** When o operador clica em um item da legenda de um gráfico do Dashboard, the system shall mostrar só as séries (ou fatias) clicadas; um novo clique no mesmo item o desmarca, e sem nenhum item marcado todas voltam. A legenda mostra o estado de cada item (marcado ou não) e a escolha vale só para o gráfico, nunca altera os dados nem os totais, e as cores de cada série não mudam ao filtrar.

### Revisão de 2026-11-08 (interface, paridade com o RAD e Backend)
- **R63 (ubiquitous):** The system shall mostrar no Dashboard, ao lado dos gráficos planos, dois histogramas em 3D (itens mais vendidos e entregas × retiradas por data), desenhados em Canvas3D com uma Scene3D de apoio, com a legenda escrita sob cada um, calculados dos mesmos dados dos cartões e na moeda escolhida (R55).
- **R64 (ubiquitous):** The system shall mostrar os indicadores do Dashboard (R52) como blocos de indicador (KPI): painel, legenda e valor, com os mesmos dados reais e a mesma moeda (R54, R55).
- **R65 (constraint):** The system shall mostrar a página em execução com as mesmas medidas que o desenhista definiu no RAD: painel do logotipo da navegação lateral (altura `HeaderHeight`, logotipo 270×80 ajustado e centrado), margens, espaçamento vertical e tamanho da fonte dos itens do menu, e o botão de abrir/recolher com a altura `BreadcrumbHeight` (41) e o ícone `BreadcrumbIconSize` (29), os mesmos valores do PowerDemo3. Diferença entre o RAD e a página é defeito, medido em pixels.
- **R66 (ubiquitous):** The system shall mostrar toda grade de dados (R60) com a linha de filtro por coluna (`ShowColumnFilters`) e altura suficiente para cinco linhas de dados além do cabeçalho, da linha de filtro e da barra de rolagem; nas telas de orçamento, as grades de linhas, de peças do catálogo e de adicionais mostram cinco linhas sem rolar.
- **R67 (event):** When uma chamada ao Backend está em andamento e vai alterar um controle da tela, the system shall mostrar nesse controle o efeito de carregamento (listras de barbearia correndo da esquerda para a direita) até os dados chegarem; o desenvolvedor só vê a propriedade "Loading effect" do controle, e qualquer comunicação que traga dados novos dispara o efeito.
- **R68 (ubiquitous):** The system shall guardar na pasta `Assets/` as imagens do projeto (logotipo da navegação e outras), referidas por caminho relativo, e adotar os programas do Backend que existam em `backend/src/` e não estejam na lista do projeto.
- **R69 (event):** When o operador para o Backend (botão Backend da barra superior do IDE ou janela do Backend), the system shall permitir executá-lo de novo com Executar, que o inicia e abre uma nova prévia; o botão fica cinza sem Backend, verde com o Backend em execução para a página aberta e vermelho quando está desatualizado ou terminou com erro, e em todo caso o clique o para.
- **R70 (constraint):** The system shall funcionar no IDE com a barra de ferramentas só de ícones (o rótulo é a dica) e a barra do desenhista 20% menor, para que os botões não se sobreponham.

### Restrições
- **R34 (constraint):** The system shall não enviar mensagem a cliente que não tenha dado consentimento para o canal.
- **R35 (constraint):** The system shall guardar chaves de gateway, email e WhatsApp apenas no Backend, nunca no formulário nem no repositório.
- **R36 (constraint):** The system shall registrar quem fez cada alteração e quando (auditoria) em cadastros, orçamentos, pagamentos e fila. Nesta versão não há login nem perfis: o nome do operador é informado na abertura da sessão e gravado na auditoria.
- **R37 (ubiquitous):** The system shall oferecer ao cliente uma página de aprovação aberta por link único e de uso limitado no tempo, onde ele vê o orçamento, aprova ou recusa e informa o motivo. O link leva um token com validade e uso único guardados no Backend, e a mensagem traz também um código curto que o cliente digita na página para confirmar a identidade.
- **R38 (ubiquitous):** The system shall expressar valores em reais (BRL) ou em euros (EUR), à escolha do consultor por orçamento, e mostrar o orçamento com os impostos discriminados por linha e, também, em versão sem impostos (subtotal), com as alíquotas mantidas em tabela por moeda.
- **R40 (ubiquitous):** The system shall manter catálogos de peças, adicionais, tarifa/hora e alíquotas com preços próprios em cada moeda (BRL e EUR), tratando cada valor na moeda em que foi cadastrado.
- **R41 (constraint):** The system shall não converter valores entre moedas em nenhuma situação (nem em orçamento, pagamento, fila ou relatório), e não somar nem comparar valores de moedas diferentes.
- **R42 (event):** When uma consulta, listagem, fila financeira ou relatório é apresentado, the system shall exigir a escolha de uma moeda como filtro e mostrar somente os registros dessa moeda, com totais por moeda.
- **R43 (ubiquitous):** The system shall fixar a moeda do orçamento na sua criação; trocar a moeda cria um novo orçamento recalculado com os preços da outra moeda, nunca converte o existente.
- **R39 (constraint):** The system shall acessar o gateway por uma interface única de cobrança, de modo que o provedor possa ser definido e trocado sem alterar o restante da aplicação.

## 5. Acceptance criteria
- [ ] AC1 — Cadastrar cliente e veículo com dados válidos grava ambos; CPF inválido é rejeitado (R1–R4).
- [ ] AC2 — Reabrir uma placa existente não cria duplicata (R3).
- [ ] AC3 — Uma coleta simulada grava data de entrada, modelo e todos os parâmetros do lote (R5, R8).
- [ ] AC4 — Interromper a coleta no meio preserva o que chegou e permite entrada manual marcada como manual (R7).
- [ ] AC5 — Para um conjunto de códigos de falha de teste, a lista de peças sugerida é a esperada pela tabela de regras (R9).
- [ ] AC6 — Alterar horas ou peças recalcula o total e registra justificativa (R10–R12).
- [ ] AC7 — Marcar e desmarcar cada um dos quatro adicionais altera o total corretamente (R13, R14).
- [ ] AC8 — O orçamento gerado mostra todos os componentes e o total é a soma conferida (R16).
- [ ] AC9 — Editar um orçamento já enviado cria a revisão 2 e preserva a 1 (R17).
- [ ] AC10 — O envio registra o resultado por canal; com um canal desligado, o outro ainda é entregue (R18, R19).
- [ ] AC11 — Orçamento vencido aparece como expirado (R20).
- [ ] AC12 — Não é possível iniciar serviço de orçamento não aprovado (R23).
- [ ] AC13 — Pagamento aprovado no ambiente de testes do gateway põe os serviços na fila; recusado não põe (R24, R26, R27).
- [ ] AC14 — Repetir a confirmação de um mesmo orçamento não gera segunda cobrança (R27).
- [ ] AC30 — Pagar em N parcelas (1 a 12; 13 é recusado), sem juros, mostra número, valor, datas (hoje, +30, +60…) e total igual ao orçamento antes de confirmar, e grava esses dados sem dados do cartão (R44, R45).
- [ ] AC43 — A primeira parcela confirmada põe os serviços na fila; as demais são cobradas nos vencimentos (data simulada) sem novo cartão (R44a, R44b).
- [ ] AC44 — Parcela futura recusada: registra a falha, avisa cliente e consultor, repete de forma limitada e nunca cobra duas vezes (R44c).
- [ ] AC45 — Sem consentimento às cobranças futuras só a opção de 1 parcela é oferecida (R44d).
- [ ] AC46 — Estorno de serviço não iniciado em pedido parcelado cancela as parcelas futuras e devolve só o já pago desse valor (R46).
- [ ] AC47 — Nenhum segredo (chave de gateway, email, WhatsApp) existe nos arquivos servidos ao navegador (R58).
- [ ] AC31 — O estorno de um serviço "na fila" é aceito pelo valor desse serviço e o retira da fila (R46, R48).
- [ ] AC32 — O estorno de serviço "em execução" ou "concluído" é recusado com o motivo e o pagamento permanece (R47).
- [ ] AC33 — Em um pedido com serviços em estados diferentes, só os não iniciados são estornados (R46, R47).
- [ ] AC34 — Estorno de pagamento parcelado, em ambiente de testes, gera o estorno no gateway e o registro local, e avisa o cliente (R46, R48).
- [ ] AC15 — Nenhum arquivo ou log contém número completo de cartão ou código de segurança (R25).
- [ ] AC16 — A conclusão do último serviço marca o veículo como pronto e avisa o cliente (R30, R33).
- [ ] AC17 — Recibo e data de retirada são enviados por email e WhatsApp após o pagamento (R31, R32).
- [ ] AC18 — Cliente sem consentimento em um canal não recebe mensagem por ele (R34).
- [ ] AC19 — Um varrimento no repositório não encontra chave de gateway, email ou WhatsApp (R35).
- [ ] AC20 — Cada alteração relevante tem registro de auditoria com usuário e hora (R36).
- [ ] AC21 — Os formulários são responsivos; `check` não reporta erro. O tema é o Elegance em versão clara (R59).
- [ ] AC35 — Percorrer todas as telas pela navegação lateral não abre janela nova nem recarrega a página, e o operador, a moeda e os filtros permanecem (R49, R50).
- [ ] AC36 — A busca global encontra um cliente, uma placa e um número de orçamento e leva à tela correspondente (R51).
- [ ] AC37 — A aplicação abre no Dashboard com os indicadores de R52 e os blocos de R53, calculados a partir de dados reais de teste e conferidos à mão (R52, R53).
- [ ] AC38 — Nenhum indicador do Dashboard aparece sem dados de origem (R54).
- [ ] AC39 — Com registros em BRL e em EUR, trocar o seletor muda todos os valores do Dashboard e nenhum total mistura moedas (R55).
- [ ] AC40 — Aprovar um orçamento ou concluir um serviço atualiza o Dashboard e a fila sem recarregar (R56).
- [ ] AC41 — Em janela estreita e em janela larga as telas se reorganizam sem sobreposição nem rolagem horizontal da página (R57).
- [ ] AC42 — A paleta e os cartões seguem a referência em todas as telas, conferido por captura de tela (R57).
- [ ] AC48 — Nenhuma tela mostra dados em colunas dentro de uma lista de uma coluna: fila, relatório, parcelas, linhas do orçamento, catálogo e listas do Dashboard são grades com cabeçalho (R60).
- [ ] AC49 — Os gráficos do Dashboard animam ao abrir e quando os valores mudam, e mostram o valor sob o ponteiro (R61).
- [ ] AC50 — Clicar em um item da legenda isola essa série ou fatia; clicar de novo desfaz; sem nenhum marcado todas aparecem; as cores não mudam e os totais do cartão permanecem iguais (R62).
- [ ] AC22 — A coleta simulada passa pela interface única de coleta; trocar a fonte não altera o restante (R5a).
- [ ] AC23 — O link de aprovação aprova, recusa com motivo e expira; o registro manual produz o mesmo estado (R21, R37).
- [ ] AC24 — Um orçamento em BRL e outro em EUR saem cada um com impostos por linha e em versão sem impostos, e os totais conferem (R38).
- [ ] AC27 — Cada peça, adicional e tarifa tem preço próprio por moeda; não existe campo de cotação nem função de conversão (R40, R41).
- [ ] AC28 — Com registros em BRL e em EUR, cada listagem e relatório mostra só a moeda filtrada, e nenhum total mistura moedas (R41, R42).
- [ ] AC29 — Trocar a moeda de um orçamento gera novo orçamento com preços da outra tabela e mantém o original intacto (R43).
- [ ] AC25 — Trocar o provedor do gateway não altera o código fora da interface de cobrança (R39).
- [ ] AC26 — O prazo sugerido muda conforme a fila e pode ser ajustado pelo consultor (R16).

- [ ] AC51 — O Dashboard mostra os dois histogramas 3D com dados, legenda sob cada um e na moeda escolhida; sem dados de origem, nenhum é desenhado (R63, R54, R55).
- [ ] AC52 — Os blocos de indicador mostram os mesmos valores do cálculo feito à mão, na moeda escolhida (R64).
- [ ] AC53 — Medidas da página em execução (altura do painel do logotipo, tamanho do logotipo, margens, espaçamento entre itens do menu, fonte, botão de abrir/recolher) iguais às do RAD, conferidas em pixels (R65).
- [ ] AC54 — Cada grade tem a linha de filtro e mostra cinco linhas de dados sem rolar; filtrar uma coluna reduz as linhas e limpar o filtro as devolve (R66).
- [ ] AC55 — Uma chamada ao Backend em andamento mostra as listras no controle marcado com "Loading effect" e as remove quando os dados chegam (R67).
- [ ] AC56 — Parar o Backend e executar de novo o inicia e abre a prévia; o botão Backend fica cinza, verde ou vermelho conforme o estado e o clique o para (R69).
- [ ] AC57 — Nenhuma pasta `.DS_Store` ou arquivo oculto impede a construção da página (R68).

## 6. Constraints & steering check
- **i18n (6 idiomas):** o texto da interface da aplicação é do projeto de exemplo, não do IDE; nenhuma string nova no `Tr` do IDE. Nomes de dados e parágrafos COBOL ficam em inglês. A interface do exemplo é em português (pt-BR), com possibilidade de outros idiomas depois.
- **Generated-code / regenerate:** projeto novo; `regenerate` e `check` limpos antes de reportar.
- **Local do projeto:** esta aplicação vive em `~/Documents/PowerOficina`, fora do repositório do PowerRustCOBOL; é um projeto construído com o PowerRustCOBOL AI, não parte dele. Nenhuma mudança no IDE, compilador ou documentação do PowerRustCOBOL é parte desta spec.
- **Capacidades ausentes:** se o `/plan` descobrir que a plataforma não oferece algo necessário (Q1), isso é registrado como relatório de lacuna para o PowerRustCOBOL, não implementado aqui.
- **Plataforma:** projeto **Web** (decisão do operador): formulários em `webforms/`, navegação no shell de aplicação (menu lateral e painel de conteúdo), construído para WebAssembly e visto no navegador; formulários responsivos. O projeto tem Backend. Segredos só no Backend (regra `backend-secrets`). O que o navegador pode executar é limitado; o `/plan` confere (Q1).
- **Tema:** Elegance claro com a paleta da referência, por decisão do operador, em lugar do Spatial que o PowerRustCOBOL AI propõe por padrão (R59).
- **Dados:** arquivos indexados (.cidx) ou SQL; decidir no `/plan`.
- **Segurança:** conformidade PCI evitada por não tocar no cartão; o formulário de pagamento é do gateway (link ou campos hospedados).

## 7. Open questions
- Q1 (resolvida no plano, 2026-11-07): a plataforma tem chamadas HTTP no Backend e segredos no Backend, mas não tem verbos de email ou WhatsApp, agendador, link de uso único, controle de calendário, Elegance claro nem eventos do servidor no navegador. Decisões: envio simulado, cobrança recorrente no gateway, token + código, grade de datas, estilo do projeto e consulta periódica. Relatórios de lacuna a registrar para o PowerRustCOBOL: agendador no Backend, verbos de email/WhatsApp, calendário, Elegance claro nativo e assinatura de eventos no navegador.

### Decididas com o operador (2026-11-07)
- Q2 Veículo: coleta **simulada** nesta versão, atrás de uma interface substituível (R5a).
- Q3 Gateway: **genérico**, provedor definido depois (R39).
- Q4 Aprovação: **link e registro manual** (R21, R37).
- Q5 Tabelas: dados **fictícios** gerados no projeto.
- Q6 Usuários: **sem login**; auditoria com nome do operador (R36).
- Q7 Prazo: **sugerido pela fila e ajustável** (R16).
- Q8 LGPD: apenas **consentimento por canal** (R34); retenção e exclusão fora desta versão.
- Q9 Moeda e impostos: **BRL e EUR**, com e sem impostos (R38).
- Q10 Euro: **sem conversão**; preços próprios por moeda e filtro obrigatório por moeda nas análises e relatórios (R40–R43). Alíquotas e arredondamento em EUR vêm de tabela fictícia de partida, como as demais.
- Q11 Parcelamento e estorno: pagamento **parcelável em até 12 vezes**; estorno **só de serviço não iniciado** (serviço "em execução" ou "concluído" não é estornável); pedido por **cliente ou consultor** (R44–R48).

- Q12 Parcelas: **sem juros**; primeira no ato da contratação e as demais a cada 30 dias (R44–R44d).
- Q14 Paleta e tema: **paleta da referência**, tema **Elegance claro** (R59).
- Q15 SPA: **Web** (R49, R58).

### Ainda em aberto (não bloqueia o plano)
- Q16: **Parcela vencida.** Se uma parcela futura não for paga depois das tentativas, o que acontece? Por exemplo: a retirada do veículo fica condicionada à quitação, ou o serviço já executado é cobrado por outro meio? A spec só exige registro e aviso (R44c).
- Q17: **Valor das parcelas.** Todas as parcelas têm o mesmo valor, ou a primeira (no ato) pode ter outro valor? A spec assume parcelas iguais, com o centavo restante na primeira.

### Lacuna da plataforma (registrada 2026-11-08)
- Os gráficos da plataforma não têm legenda clicável nativa (nem no desktop nem no navegador). Até haver, R62 é atendido na tela com botões de legenda próprios, que redesenham o gráfico só com o que foi marcado; relatório de lacuna: docs/compiler-requests/chart-interactive-legend.md.
