# Lista de recursos previstos — série WASM (linha 2.x.x)

**Data:** 2026-10-20
**Fonte:** `specs/WASM-*` do branch `features-2x` (worktree `.claude/worktrees/features-2x`, HEAD `c19bcee5`), mais `WASM-INDEX.md`, `WASM-PLAN.md`, `WASM-ABI-v1.md`, `WASM-PROGRESS.md` e `WASM-PARITY.md`.
**Método:** leitura integral de cada documento. O código-fonte não foi consultado. Cada spec teve a contagem de requisitos R conferida com `grep -cE '^- \*\*R[0-9]+[a-z]?\b'`.
**Escopo deste arquivo:** o que os specs dizem que será feito. Ser previsto não quer dizer ser implementado. A situação real de cada item está na seção 7.

---

## 1. Números

| Item | Quantidade |
|---|---|
| Specs escritos (WASM-001 a WASM-018) | 18 |
| Requisitos R (EARS) somados nos 18 specs | 322 |
| Specs propostos, sem arquivo (WASM-019 a WASM-035) | 17 |
| Decisões D1 a D19 (WASM-INDEX §3) | 19 |
| Itens de WASM-PLAN.md | 85 |
| Itens de WASM-ABI-v1.md | 152 |
| Itens de WASM-PROGRESS.md | 86 (2 deles são referência) |
| Divergências de WASM-PARITY.md | 10 |

Requisitos R por spec:

| Spec | R | Spec | R |
|---|---|---|---|
| WASM-001 | 18 | WASM-010 | 20 |
| WASM-002 | 20 | WASM-011 | 16 |
| WASM-003 | 23 | WASM-012 | 14 |
| WASM-004 | 24 | WASM-013 | 17 (inclui R15b) |
| WASM-005 | 18 (inclui R5b) | WASM-014 | 17 |
| WASM-006 | 15 | WASM-015 | 19 (inclui R8b, R17 e R18) |
| WASM-007 | 16 | WASM-016 | 21 (inclui R13b, R13c, R18 e R19) |
| WASM-008 | 11 | WASM-017 | 20 |
| WASM-009 | 13 (inclui R8b) | WASM-018 | 20 |

---

## 2. Índice temático

Para achar um recurso pelo assunto. O detalhe fica na seção 3.

| Tema | Specs |
|---|---|
| Tipos de projeto, seletor New Project, árvore da IDE, manifesto | 001 |
| Build Web e Mobile, host page, portable subset, secret scan | 002 |
| Backend (binário, WebSocket, JWT, limites, logs, scaffold) | 003, 014 |
| Reuso do catálogo de controles, classes A a E | 004 |
| `CALL … WITH WEBSOCKET LINKAGE`, mapeamento JSON, controles de dados remotos | 005 |
| Controle WebLink | 006 |
| CSS3, `CssClass`, `Style`, stylesheet da form | 007 |
| Preview em web view | 008 |
| Controle Authentication, provedores de identidade, reCAPTCHA | 009 |
| Gerador WebAssembly (layout, aritmética, fluxo, host API, suspensão) | 010 |
| Navegação SPA e standalone, rotas, `super`, pilha mobile | 011 |
| Shell mobile Tauri 2 (Android, iOS, assinatura, permissões, recursos de dispositivo) | 012 |
| Segurança (threat model, CSP, XSS, CSRF, supply chain, logs) | 013 |
| Contrato do backend (versões, erros, idempotência, streams, arquivos) | 014 |
| Testes, ferramentas MCP, plugin, System KB, Grace, exemplos | 015 |
| Player pré-compilado, form description, reconexão, versões | 016 |
| Arquitetura do módulo (um por projeto, regiões, multi-memory, threads, fronteira JS) | 017 |
| Canvas2D, Canvas3D, ImageBuffer, Plot, Scene3D, Physics2D, Physics3D | 018 |
| Contratos de baixo nível (imports, exports, batch, manifest, verbos `COBOL::`) | WASM-ABI-v1 |
| Ordem de trabalho, crates, riscos, definição de pronto | WASM-PLAN |
| Divergências entre o gerador e o interpretador | WASM-PARITY |

---

## 3. Inventário por spec

Convenções: **R** é um requisito do spec. **Q** é uma questão em aberto; quando ela cria ou escolhe um recurso, o status (decidido, aberto ou recomendado) vem junto. **Fora do escopo** são os non-goals, e **Estado** diz o que o próprio spec ou o plano registra sobre a implementação.

### WASM-001 — Tipos de projeto web e mobile, seletor New Project e entradas da IDE

Status do spec: rascunho, precisa de `/clarify`. Não depende de nada e é pré-requisito dos demais.

**Requisitos**

- **R1** — Diálogo New Project com a lista de templates à esquerda e, à direita, *Project Properties* e *Template-Specific Options* do template escolhido, como no mock-up. Trocar de template muda o título do diálogo e as opções.
- **R2** — A lista de templates é dado, não código de desenho: cada template é um descritor (id, nome, plataformas, project kind, opções com defaults, se pode ser criado). Acrescentar um template não muda o código do diálogo.
- **R3** — Para todo template o diálogo coleta project name, version, main/entry file, project location, description, UI style/theme e default language/locale. Os valores vão para o manifesto e para `[ide]` e `[forms]`.
- **R4** — Cada template tem suas próprias opções, e uma opção que não se aplica não aparece. Exemplos: Desktop tem window style e UI framework; Mobile tem default orientation e minimum OS version; High DPI e GPU acceleration só existem no Desktop; Web e Mobile têm **application style** (SPA ou standalone, WASM-011 R1).
- **R5** — Enquanto as features de um template não existem, a entrada aparece marcada como indisponível e *Create* fica desabilitado. Criáveis na 2.0: Desktop, Web (WebAssembly), Android (Cellphone), Android (Tablet), iPhone e iPad. Os demais são indisponíveis.
- **R6** — *Create* cria as pastas e os arquivos iniciais do kind e abre o projeto. Em Web e Mobile cria também o scaffold do backend (WASM-003 R22), salvo se o desenvolvedor desmarcar a opção (Q5).
- **R7** — Opcional: *Recent Projects* e *Create from Template…*. Nenhum dos dois é necessário para criar um projeto.
- **R8** — O project kind (`desktop`, `web`, `mobile`) é gravado no manifesto e recarregado com o projeto. Em Mobile também ficam gravados a plataforma e a device class. Manifesto sem kind carrega como desktop.
- **R9** — IDE, leitor de manifesto do compilador e leitor de manifesto da CLI concordam nos campos novos. `project_upgrade` leva manifestos antigos adiante sem perder campo.
- **R10** — A ferramenta MCP `create_project` aceita o kind e suas opções. IDE e ferramenta produzem arquivos idênticos para as mesmas entradas, e o teste de byte-identity cobre os kinds novos.
- **R11** — Projeto desktop aberto: a árvore mostra exatamente as entradas de hoje.
- **R12** — Projeto web aberto: a árvore mostra **WebForms** e **Backend** no lugar de Forms. Projeto mobile: **MobileForms** e **Backend**. Indexed Files, SQL Connections, Common Code, Generated, External Crates, Assets e Documentation continuam (Q1).
- **R13** — Cada entrada nova se comporta como Forms: `[+]` cria um item do tipo, há uma pasta em disco, ícone, rótulo nos seis idiomas, e lugar na desambiguação do Grace, no walkthrough e na listagem do agent kit.
- **R14** — Com projeto web ou mobile aberto, a toolbox oferece só os controles do kind (WASM-004 R5). Run, Build e Preview da toolbar fazem o que WASM-002 e WASM-008 definem.
- **R15** — O IDE continua sendo uma única aplicação: sem executável novo, sem modo de janela novo, sem licença nova.
- **R16** — Toda string do diálogo e das entradas novas é campo `Tr` com seis idiomas, incluindo nomes e descrições de template, rótulos de opção e notas de indisponibilidade.
- **R17** — O diálogo tem tamanho explícito e não se redimensiona sozinho. Um único grip é o escritor desse tamanho.
- **R18** — Projeto já em disco abre como antes. Nenhum arquivo é reescrito só por ser aberto.

**Metas:** kind gravado no manifesto e respeitado pela IDE inteira (`desktop`, `web`, `mobile`); o Backend **não** é kind, é a pasta `backend/` de todo projeto Web ou Mobile (operador, 2026-10-20); diálogo guiado por dados; resultado igual na IDE, no `rcrun` e no MCP.

**Fora do escopo:** build e execução web/mobile (WASM-002, WASM-008); o backend em si (WASM-003); o catálogo de controles (WASM-004); alvos watch (Android Smartwatch, Apple Watch) e os templates Agentic AI Chatbot (Web) e UserControl (Web), que aparecem como indisponíveis; mudar o kind depois da criação (Q7); marketplace de templates.

**Mock-up do operador (fixa layout e campos, não o texto):** diálogo *New Desktop Project*; lista com All Templates, Desktop (macOS, Windows, Linux), Agentic AI Chatbot (três SOs e Web), Web (WebAssembly), UserControl (Desktop e Web), Android (Cellphone e Tablet), Android (Smartwatch), iPhone, iPad, Apple SmartWatch e Recent Projects; abas Project Properties e Template-Specific Options; rodapé com Create from Template…, Recent Projects, Cancel e Create. Observação: R5 diz que só as seis entradas da Q3 são criáveis, então Chatbot (desktop) e UserControl (Desktop) também ficam indisponíveis.

**Questões:**
- **Q1** — Entradas de árvore por kind — recomendado: Web = WebForms + Backend; Mobile = MobileForms + Backend; Desktop = Forms. Alternativa: mostrar as três e desabilitar as que não se aplicam.
- **Q2** — Formato de arquivo — decidido (operador, 2026-10-20): forms web com `.wcfrm` e forms mobile com `.mcfrm`; desktop continua `.cfrm`.
- **Q3** — Templates indisponíveis e conjunto criável na 2.0 — **decidido** (operador, 2026-10-19): Web + Android + iOS, sem watch, com a lista de R5.
- **Q4** — "Target platform" do Desktop — recomendado: mostrar o host, desabilitado, até haver cross-build.
- **Q5** — Scaffold do backend sempre criado? — recomendado: criado por padrão, com opção de desmarcar.
- **Q6** — *Recent Projects* e *Create from Template…* — decidido (operador, 2026-10-20): *Create from Template…* copia um projeto salvo; *Recent Projects* não existe ainda, segundo o operador (a confirmar: o código já tem uma lista de recentes no diálogo).
- **Q7** — Kind imutável? — recomendado: fixo na 2.0; conversão em spec posterior.
- **Q8** — Nomes de pasta — recomendado: `webforms/`, `mobileforms/`, `backend/`.

**Estado (plano):** registrado como implementado no branch `worktree-agent-a97ba65c65571c2df`, depois mergeado (PROGRESS PROG-81, 2.0.20). Modelo em `cobolt-compiler/src/project_kind.rs`; criação em `cobolt-project-tools` (`create::plan`, usado pela IDE e pela ferramenta); diálogo em `cobolt-ide/src/panels/new_project.rs`; `[desktop]` é gravado mas não tem efeito em execução ainda. **Não terminado:** filtro da toolbox por kind (R14, aguarda WASM-004); scaffold do backend só cria `HEALTH` (R6); Target platform do Desktop mostra o host e não é escolha (Q4).

---

### WASM-002 — Alvo WebAssembly (Leptos e Tauri 2) e host page

Status: rascunho. A execução foi decidida (operador, 2026-10-19): o compilador gera WebAssembly direto (WASM-010), sem interpretador nem runtime no browser.

**Requisitos**

- **R1** — Build de projeto Web produz um **static bundle** em `dist` (specs 100/101 governam pasta e cache): `index.html` do host; o **módulo de programa** gerado, um para o projeto inteiro, em variante multi-memory e single-memory (WASM-010, WASM-017 R1 e R9); o **player pré-compilado** (WASM-016), plain e threaded, copiado e nunca compilado; o loader JS; as descrições das forms; a stylesheet (WASM-007); os assets. Nomes com hash do conteúdo. Em standalone, cada form ganha uma entry page.
- **R2** — Build de projeto Mobile produz um **pacote Android** e/ou um **pacote iOS** que embrulham o mesmo frontend num shell Tauri 2 (WASM-012).
- **R3** — Check ou Build de projeto Web/Mobile valida o COBOL contra o **portable subset**. Cada construção que não roda no browser vira erro na sua linha, nomeando a construção e a alternativa. Enquanto houver erro, nenhum bundle é produzido. Não portáveis: qualquer organização de arquivo (inclusive indexed e `redb`), processo filho, thread de SO, socket nativo, SQL, `EXEC RUST`, secret embutido. Quem recusa é o gerador (WASM-010 R8).
- **R4** — Para cada construção do subconjunto, o programa se comporta como no desktop: mesmos resultados, mesmos desfechos de `ON SIZE ERROR` e `ON EXCEPTION`, mesma ordem de eventos. Toda diferença é listada num único lugar e a igualdade é medida (WASM-010 R16).
- **R5** — A UI é DOM com CSS3, não `<canvas>` (seleção de texto, zoom, acessibilidade e find-in-page funcionam). Exceção: os controles de desenho de classe B (charts, maps, gauges) podem usar SVG ou canvas dentro do próprio elemento.
- **R6** — Toolchain ausente: o IDE nomeia a ferramenta, diz se pode instalá-la e só instala o seguro, com consentimento (`rustup target add` sim; Xcode e Android SDK não). Build **Web** não precisa de toolchain Rust. Build **Mobile** precisa de Tauri 2 CLI, Android SDK/NDK, Xcode e toolchain Rust (um shell por aplicação).
- **R7** — O bundle não contém nenhum secret (key, token, password, connection string, client secret). O Build varre arquivos, forms e stylesheets, e falha nomeando arquivo e padrão.
- **R8** — O Build reporta o tamanho de cada parte (módulo, loader, stylesheet, assets).
- **R9** — Projeto Desktop compila como antes. Nada muda `bin/`, eframe nem o host triple.
- **R10** — Opcional, **modo janela**: a página faz a aplicação preencher o viewport (inclusive o dynamic viewport de telefone), sem scroll bars próprias, e refaz o layout quando a janela muda.
- **R11** — Opcional, **modo dentro de layout existente**: a página monta a aplicação num elemento escolhido pelo host (atributo ou seletor), dimensiona pelo elemento e refaz o layout quando o elemento muda de tamanho. Breakpoints dos forms são avaliados pela largura do elemento.
- **R12** — Opcional, **modo embutido**: o CSS da página não vaza para a aplicação, nem o da aplicação para fora. O host pode passar valores de tema por CSS custom properties.
- **R13** — Superfície JS para o host, pequena e documentada: mount, unmount; parâmetros iniciais (locale, theme, backend address); eventos de saída (ready, error, navigated, signed in/out). Não oferece meio de avaliar script arbitrário.
- **R14** — A aplicação roda sob CSP com `'wasm-unsafe-eval'` e sem `'unsafe-eval'`. O Build imprime a política necessária (script, style, font, image e connect).
- **R15** — Durante o carregamento, a página mostra progresso. Sem WebAssembly, ou com falha de carga, mostra mensagem legível e botão de tentar de novo; nunca página em branco.
- **R16** — Cada módulo de programa é autocontido (WASM-010 R1–R2) e importa só a host API. O bundle não tem interpretador, AST nem runtime.
- **R17** — A host page serve os dois estilos de WASM-011: **SPA** (uma entry page e um router) e **standalone** (uma entry page por form). O modo embutido suporta SPA; o suporte a standalone é decisão de WASM-011.
- **R18** — O Build declara um **baseline de features WebAssembly** e uma **matriz de browsers** (Chrome e Edge, Firefox, Safari, iOS web view, Android WebView). O loader sonda o browser contra ela e mostra a mensagem de R15 abaixo do baseline. Módulo multi-memory por padrão e single-memory para iOS e matrizes com Safari. Player threaded e plain. O baseline é o módulo single-memory com o player plain.
- **R19** — O loader busca o módulo com `fetch` e instanciação em streaming; integração ES-module só por feature-detection (WASM-017 R3). Busca as partes pesadas do player no primeiro uso. Cria a região de dados de cada form na criação e a libera na destruição (WASM-017 R2–R5). Escolhe variante do módulo, do player e modo de execução (Worker ou main thread) por sonda.
- **R20** — **Threads só quando a página é cross-origin isolated**, e a aplicação funciona sem elas. O Build gera, para cada static host documentado e para o servidor standalone do IDE, os headers `Cross-Origin-Opener-Policy: same-origin` e `Cross-Origin-Embedder-Policy` (`require-corp` ou `credentialless`), e imprime o trade-off. Pode oferecer um service-worker shim que define os headers (desligado por padrão). No modo embutido, a aplicação só é isolada se a página host for.

**Metas:** Build Web como static bundle; Build Mobile como pacote Android e iOS; mesmo comportamento para o que o subconjunto permite; checagem do subconjunto em Check e Build, por linha; host page com modo janela e modo embutido e superfície JS pequena; detecção de toolchain honesta.

**Fora do escopo:** runtime, interpretador e `redb`, disco ou threads no browser (o backend os mantém); SSR, PWA e offline; hospedagem ou deploy; debugger web dentro do IDE; mudar o build desktop; o backend (WASM-003).

**Questões:**
- **Q1** — Modelo de execução — **decidido** (operador, 2026-10-19): gerador direto, sem interpretador nem runtime no browser.
- **Q2** — Bundler e framework — recomendado: Leptos em client-side rendering com `trunk`. Alternativa: `cargo-leptos`.
- **Q3** — Tauri 2 também para Desktop? — recomendado: não, Desktop continua em egui na 2.0.
- **Q4** — `EXEC RUST` em projeto web — **decidido** pelo modelo: rejeitado; permitido depois só via backend ou spec de external crates.
- **Q5** — Orçamento de tamanho e startup — aberto: definido no `/clarify`, depois de medir a fase A de WASM-010.
- **Q6** — JavaScript da host page — recomendado: escrito à mão e entregue como arquivo fixo. Alternativa: gerado.
- **Q7** — O player — **decidido** (operador, 2026-10-19): dois `.wasm`, um player Leptos pré-compilado e o módulo COBOL. Custo aceito: sem código Leptos por projeto, sem controles `Custom` na web na v1.

**Estado:** rascunho. Itens de execução em WASM-010, WASM-016 e WASM-017.

---

### WASM-003 — Projeto Backend e canal seguro frontend–backend

Status: rascunho. Depende de WASM-001. É pré-requisito de WASM-005 e WASM-009.

**Requisitos**

*Projeto e build*
- **R1** — O Backend (a pasta `backend/` de um projeto Web ou Mobile) guarda programas COBOL, copybooks, definições de indexed file, SQL connections e uma lista de **exported programs**. Só eles são alcançáveis remotamente, qualquer que seja o nome do programa.
- **R2** — Build do Backend produz **um executável autocontido por plataforma host** (macOS, Windows, Linux), com runtime, programas e servidor WebSocket, estendendo o build de binário único.
- **R3** — Os programas rodam no **runtime nativo atual**: interpretador, `std::fs`, threads, todos os engines indexed inclusive `redb`, com arquivo, indexed, SQL e HTTP, mais WebSocket, validação de JWT e o mapeamento JSON de WASM-005. O backend não é compilado pelo gerador de WebAssembly.
- **R4** — Ao compilar, o backend grava uma **interface description** de cada exported program (nome, grupos de parâmetro, grupo `RETURNING`, roles exigidos) que o projeto frontend importa (WASM-005 R7).

*Canal*
- **R5** — Canal **WebSocket sobre TLS** (`wss`). `ws` simples só para endereço loopback em execução de desenvolvimento.
- **R6** — Cada requisição tem id, nome do exported program e payload; cada resposta tem o mesmo id e o resultado ou erro com classe (WASM-005 R10). Requisições simultâneas e respostas fora de ordem são permitidas.
- **R7** — Queda de conexão: o chamador é avisado (a chamada falha). Nada se perde em silêncio, e nada roda duas vezes por reconexão, salvo programa declarado seguro para repetir (Q4).
- **R8** — Keep-alive periódico, derrubada de conexões mortas, e reconexão com back-off no cliente.

*Autenticação e autorização*
- **R9** — Bearer JWT válido é exigido antes de qualquer exported program, exceto os declarados **public** (conjunto pequeno: os programas de sign-in de WASM-009 e um health check).
- **R10** — Validação de assinatura contra as chaves publicadas do emissor. Allow-list de algoritmos (nunca `none`). Checa issuer, audience, expiry e not-before com clock skew limitado. Aceita só access token, não identity token.
- **R11** — O token nunca aparece em URL, query string nem log. O meio de entregá-lo no WebSocket é decisão de `/plan`.
- **R12** — Claims validadas (subject, roles, tenant) são lidas pelo programa por built-in inline `COBOL::"…"`, somente leitura. Um exported program pode declarar roles exigidos, e o backend recusa os outros antes de o programa começar.
- **R13** — Token que expira com a conexão aberta: a próxima requisição recebe *authentication expired*, e o frontend renova (WASM-009 R5) e repete.

*Hardening*
- **R14** — Checagem do **Origin** do WebSocket contra allow-list configurada (contra cross-site WebSocket hijacking).
- **R15** — Todo payload é validado contra o layout do programa-alvo (nomes, tipos, comprimentos, limites de tabela) antes de rodar. Campo desconhecido, tipo errado ou valor longo demais são recusados, sem truncamento silencioso.
- **R16** — Tamanho máximo de frame, timeout por chamada, rate limit por conexão e por usuário, e limite de chamadas concorrentes. Configuráveis, com defaults seguros.
- **R17** — Todo secret (senhas de banco, API keys, client secrets, chaves de assinatura) fica na configuração do backend ou no secret store do SO. Nunca em form, copybook ou arquivo do frontend.

*Concorrência e estado*
- **R18** — O estado de runtime de uma chamada é isolado do de outro chamador. Q2 decide se `WORKING-STORAGE` é novo a cada chamada.
- **R19** — Com mais de um cliente usando os mesmos dados indexed ou SQL, valem as semânticas de lock **entre conexões**, não só dentro de um run unit. O gap de cross-process locking é pré-requisito, e é um **fix** pela decisão de 2026-09-14, em 1.x.x e portado para 2.x.x.

*Operação*
- **R20** — Iniciar, parar e acompanhar o backend pelo IDE, com log no painel Output. O mesmo binário fora do IDE se comporta igual.
- **R21** — Logs estruturados (call id, program, caller subject, duration, outcome), nunca com token, senha ou campo sensível. Responde a uma sonda de health.
- **R22** — Projeto Web ou Mobile novo nasce com um scaffold de Backend contendo os programas de autenticação de WASM-009 (WASM-001 R6).
- **R23** — O frontend só conecta aos endereços de backend configurados para ele, e o build imprime o `connect-src` que isso implica (WASM-002 R14).

**Metas:** Backend como pasta de todo projeto web ou mobile, com exported programs e binário desktop; WSS com servidor que só serve programas exportados; autenticação por chamada com JWT e autorização por role; canal seguro por padrão; ciclo de desenvolvimento pela IDE.

**Fora do escopo:** deploy em nuvem, containers, escala (propostos em WASM-019); fluxos de identity provider (WASM-009); sintaxe e mapeamento JSON (WASM-005); API REST pública, GraphQL ou server push; multi-tenant.

**Questões:**
- **Q1** — Onde o backend é declarado — **decidido** (operador, 2026-10-20): pasta `backend/` dentro do projeto, não projeto separado.
- **Q2** — Estado — recomendado: `WORKING-STORAGE` novo a cada chamada, com sessão em arquivo ou banco. Alternativa: instância por conexão.
- **Q3** — Endpoints HTTPS além do WebSocket (cookie de refresh, health, redirect do provedor) — aberto, "provavelmente sim".
- **Q4** — Idempotência — recomendado: nenhum programa é seguro para repetir por padrão.
- **Q5** — Token de desenvolvimento (JWT local para testar sem provedor) — aberto, precisa de ruling: só em execução de desenvolvimento, nunca em release?
- **Q6** — Empacotamento — recomendado: um binário por host em 2.0.
- **Q7** — TLS — recomendado: terminação pelo binário ou por reverse proxy; o binário recusa `ws` não-loopback nos dois casos.

**Estado:** rascunho. O backend ainda não foi mergeado (PROGRESS PROG-79).

---

### WASM-004 — Reuso do catálogo de controles na web

Status: rascunho. Depende de WASM-001, 002, 010 e 016. Emenda a spec 017: na web o DOM é o renderer.

**Requisitos**

*Uma definição*
- **R1** — Um controle de form web ou mobile é o mesmo controle do desktop, guardado em `.wcfrm` (web) ou `.mcfrm` (mobile): mesmos tipos, propriedades, eventos e métodos. Propriedades só da web (`CssClass`, `Style`, WASM-007) entram no modelo como as demais.
- **R2** — Cada controle tem uma **classe primária** (A a E) num registro único lido por toolbox, Check, KB e agent kit. Onde operações diferem, o registro lista essas operações e a classe que assumem: Maps é B para imagem e C para geocode e directions; Viewer é B para páginas, C para open/save e D para print/share; ToolBar é A para o visual e D para ações que iniciam programa.

*Comportamento*
- **R3** — Controle classe A é renderizado com o elemento HTML nativo (`button`, `input`, `select`…), CSS de suas propriedades (WASM-007), e dispara os mesmos eventos na mesma ordem.
- **R4** — Controle classe B é desenhado com SVG ou canvas, aceita as mesmas propriedades de dados e métodos, e documenta por controle onde a imagem difere do desktop.
- **R5** — Com projeto web ou mobile, a toolbox esconde os controles classe D, e o Check reporta um controle D num form (por exemplo copiado de um form desktop) com linha e alternativa.
- **R6** — Painel Properties, toolbox e editor de event-binding ficam iguais para um controle web. Propriedade sem efeito ou só aproximada na web (perspectiva `RotateX`/`RotateY`, blur do SO) continua editável, para o form ir e voltar entre kinds, e é marcada como approximate ou ignored no tooltip (Q3).
- **R7** — Todo membro só da web entra no vocabulário fechado de métodos (`cobolt-ast/src/methods.rs`) e nas tabelas do System KB, para não ser lido como subscript.

*Layout e entrada*
- **R8** — Os quatro modos de layout e os breakpoints são honrados por **CSS layout** (operador, 2026-10-19): Flex, Grid e Flow mapeiam para flex, grid e flow; Absolute para posicionamento absoluto; breakpoints para media ou container queries. A paridade com o solver desktop é medida em R11.
- **R9** — Toque dispara os mesmos eventos de clique que o mouse. Eventos que dependem de hover são documentados como não disparados em toque. O modelo completo de interação mobile é WASM-021 (proposto).
- **R10** — Controle operável por teclado, com nome acessível e role corretos: os do elemento nativo ou o padrão ARIA dos compostos (TreeView, TabControl, DataGrid, SideMenu, MenuBar).

*Garantia*
- **R11** — Todo controle A e B tem teste web que põe cada propriedade num valor distinto e lê de volta do que o browser mostra. Há também checagem de paridade de geometria em browser headless contra o engine desktop, num form de referência com pelo menos um controle de cada classe.
- **R12** — O System KB declara a classe de cada controle e os membros só da web, e `chunked.data` regenerado os carrega (freshness é gate real).

**Contexto:** o catálogo tem 62 tipos de controle hoje e 69 com WASM-018 (Canvas2D, Canvas3D e cinco client objects).

**Classificação proposta (a confirmar no `/clarify`):**
- **A — DOM nativo:** Button, TextBox, Label, CheckBox, RadioButton, ListBox, ComboBox, GroupBox, Panel, TabControl, ProgressBar, StatusBar, Line, Shape, NumericUpDown, Slider, Switch, Splitter, TreeView, MenuBar, SideMenu, ToolBar (parte visível), DateTimePicker, PictureBox, Timer, Snackbar.
- **B — redesenhados (SVG ou canvas):** BarChart, LineChart, PieChart, AreaChart, ScatterChart, DonutChart, RadarChart, Knob, Gauge, Animator, Maps (tiles), Viewer (PDF, subconjunto HTML, Mermaid), e os novos **Canvas2D** e **Canvas3D** (WASM-018).
- **C — via backend:** DataGrid (bindings IndexedFile e SQL), IndexedFile, SqlDatabase, KnowledgeBase, RestClient, AgentObject, WebSearch, FileDropZone, Maps (geocode e directions), Viewer (open e save) e os 16 controles AWS.
- **D — indisponíveis:** ações de ToolBar que iniciam programa, abrem terminal ou imprimem pelo SO; Viewer print e share; controles plug-in `Custom` (WASM-032).
- **E — client objects** (sem visual, rodam no player ou no host egui): os novos ImageBuffer, Plot, Scene3D, Physics2D e Physics3D (WASM-018).

**Pontos de toque** listados para os planejadores (§6): `model.rs` (enum, `ALL`, `as_str`, `from_str`, `default_size`, `primary_event`, `supported_events`, `is_non_visual`, `Control::new`, `runtime_property_names_for`), `paint.rs`, `render.rs`, `methods.rs`, os arms de método do interpretador, o seeding do form host, `runtime_features.rs`, as tabelas de KB do compilador, toolbox, `properties.rs`, `designer.rs`, `agent.rs`, `prop_help_data.rs`, `ide_text_data.rs` e `i18n.rs`.

**Metas:** uma definição por controle; todo controle classificado e publicado; controles A e B iguais ao desktop, com diferenças listadas; harness de conformidade web.

**Fora do escopo:** controles novos (WebLink é WASM-006, Authentication é WASM-009); redesenho visual; levar o canvas do Form Designer para a web.

**Questões:**
- **Q1** — Layout na web — **decidido**: CSS (R8). Diferença de um pixel medida em R11.
- **Q2** — Classe dos controles limítrofes (Animator, Timer, DateTimePicker, TreeView, DataGrid) — aberto, decidir por controle no `/plan`.
- **Q3** — Propriedade sem efeito na web — recomendado: manter e marcar (R6). Alternativa: esconder.
- **Q4** — DataGrid — aberto, com direção indicada: manter `Rows` (`\n` e TAB) e `Columns` (`Name:Type`) para uso em memória. Tabela remota grande precisa de paginação no servidor (controles de dados além da v1, WASM-023).
- **Q5** — Fontes — recomendado: system stack, com fontes do projeto por `@font-face` a partir de `Assets/`.
- **Q6** — Visual "egui-elegance" (Knob, sombras de vidro, blur do SO, cantos da spec 057) — aberto: reproduzir em CSS ou aceitar como aproximado.

**Estado:** rascunho. Pré-requisito do player (WASM-016) e de WASM-005, 006, 007 e 008.

---

### WASM-005 — `CALL … WITH WEBSOCKET LINKAGE` e controles de dados pelo backend

Status: rascunho. Depende de WASM-002, 003, 004, 010 e 016. Necessário para WASM-009.

Sintaxe pedida pelo operador: `CALL "<nome>"|identificador WITH WEBSOCKET LINKAGE USING BY VALUE|BY REFERENCE <grupo> [RETURNING <grupo>]`. Grupos viram JSON pela estrutura de dados. O encanamento, incluindo o token, é invisível ao desenvolvedor.

**Requisitos**

*A linguagem*
- **R1** — `CALL {literal | identifier} WITH WEBSOCKET LINKAGE USING [BY VALUE | BY REFERENCE] group … [RETURNING group] [ON EXCEPTION …] [NOT ON EXCEPTION …] [END-CALL]` entra na AST como convenção de chamada. Todo `CALL` existente continua igual.
- **R2** — Cada argumento é um **group item** declarado em `WORKING-STORAGE` (Q1 sobre itens elementares). Qualquer outra coisa é recusada com diagnóstico na linha.
- **R3** — Modos: **BY VALUE** (o grupo é enviado, a cópia do chamador não muda); **BY REFERENCE** (os valores finais do chamado são escritos de volta); **RETURNING** (o item `RETURNING` do chamado vai para o grupo nomeado). Podem ser combinados.
- **R4** — Chamada `WEBSOCKET` sem `ON EXCEPTION` é aceita se a interface description importada declara o nome; o analisador reporta o nome que ela não declara.

*Mapeamento*
- **R5** — Grupo vira objeto JSON com membros na hierarquia e ordem declaradas. Regras fixadas por teste:
  - **Nomes:** chave é o data-name como declarado. `FILLER` e nomes de nível 88 são omitidos (Q3).
  - **Tabelas:** `OCCURS n` vira array; `OCCURS … DEPENDING ON` tem o número corrente de ocorrências.
  - **Números:** número JSON escrito a partir do texto decimal com a escala do `PICTURE`, nunca por ponto flutuante binário, com sinal. Número que um JavaScript não guarda exato é escrito por inteiro e lido como decimal.
  - **Texto:** string JSON em UTF-8; espaços à direita cortados na saída (Q4).
  - **USAGE:** `COMP`, `COMP-3` e similares vão como valor numérico, nunca como bytes.
  - **REDEFINES:** só o item redefinido é escrito (Q5).
- **R5b** — O mapeamento é implementado **duas vezes, pelas mesmas regras**: no código que o gerador embute no módulo (WASM-010 R6) e no runtime nativo do backend. Uma tabela normativa e um conjunto de **golden vectors** (grupo, texto JSON, grupo lido de volta) rodam nas duas implementações.
- **R6** — O JSON é lido **por nome, não por posição**. Chave ausente deixa o item como está. Chave desconhecida, tipo errado, número que não cabe no `PICTURE`, string maior que o `PIC` ou tabela maior que o `OCCURS` são **erro**, nunca truncamento. Ida e volta é idêntica.

*Contrato remoto*
- **R7** — Layouts dos argumentos são compartilhados por copybooks (`COPY`). A interface description do backend é importável pelo projeto frontend, e a checagem de R4 e de compatibilidade de layout roda no Check.
- **R8** — O desenvolvedor não escreve conexão, token, serialização, framing, correlação, keep-alive, reconexão ou refresh. Token expirado é renovado e a chamada repetida uma vez antes de falhar.
- **R9** — Com chamada `WEBSOCKET` pendente, o form continua responsivo, mas o comando depois do `CALL` não roda. Eventos novos entram numa fila em ordem (Q6). No browser isso é o pause-and-resume do módulo (WASM-010 R9).
- **R10** — Falhas (sem conexão, timeout, falha ou expiração de autenticação, validação, erro remoto, função desconhecida): o `ON EXCEPTION` roda, e a classe e a mensagem são lidas por built-in inline `COBOL::"…"` (Q7).
- **R11** — Timeout e payload máximo com defaults seguros, configuráveis por projeto. O backend impõe seus limites (WASM-003 R16).

*Os controles*
- **R12** — Todo controle classe C (WASM-004) mantém sua API COBOL na web. Diferenças visíveis: latência e os modos de falha de R10.
- **R13** — DataGrid com binding IndexedFile ou SQL faz refresh, edit, save e auto-commit pelo backend. Binding CobolTable fica no browser. Os binding kinds `RestApi` e `AgentAi`, modelados mas nunca executados, são implementados pelo backend ou retirados da web (Q8).
- **R14** — Controles com conector ou secret (SqlDatabase, RestClient, AgentObject, KnowledgeBase, WebSearch, controles AWS, Maps geocode e directions) rodam no backend. O frontend guarda só o **nome** da configuração, nunca connection string, key ou password. O Build falha se houver.
- **R15** — FileDropZone e Viewer movem arquivos com limite de tamanho e allow-list de tipos. O backend nunca aceita caminho escolhido pelo browser.
- **R16** — Para cada controle classe C, o build gera **as duas pontas**: as chamadas do frontend e os exported programs do backend, a partir da configuração do controle. Os endpoints exigem autenticação e podem ser restritos por role (padrão: só autenticado).
- **R17** — O backend só executa statements e operações declarados em design time (query nomeada, operação de arquivo com parâmetros ligados). Nunca executa SQL, comando, caminho ou URL vindo do browser.

**Metas:** `CALL … WITH WEBSOCKET LINKAGE` com grupos em `USING`, `BY VALUE`/`BY REFERENCE` e `RETURNING`; mapeamento grupo ⇄ JSON sem perda; encanamento invisível; controles classe C funcionando no browser com APIs inalteradas; nenhum texto SQL, secret ou caminho do browser executa no backend.

**Fora do escopo:** `JSON GENERATE` e `JSON PARSE` gerais (possível spec posterior); streaming, server push, payload binário e protocolo de arquivo grande (WASM-014, propostos); mudar a semântica do `CALL` local; transporte diferente do WASM-003.

**Questões:**
- **Q1** — Itens elementares como argumento — recomendado: permitir `01`/`77` elementar.
- **Q2** — `RETURNING` em chamada local — **decidido e feito**: fix em 1.x.x portado para 2.x.x (`8a15f00d` e `afe22c51`).
- **Q3** — Grafia das chaves JSON — recomendado: como declarado. Sobrescrita por item pela cláusula `NAME`.
- **Q4** — Trimming — recomendado: cortar à direita na saída, preencher na entrada. Espaços à esquerda e valor só de espaços precisam de regra.
- **Q5** — `REDEFINES` — recomendado: escrever só o primeiro item. Aberto: recusar grupo `REDEFINES` como argumento?
- **Q6** — Eventos com chamada pendente — recomendado: enfileirar.
- **Q7** — Como a falha é lida — aberto (precisa do gosto do operador): built-in como `COBOL::"WS-LAST-ERROR"( class message )`, special register ou `RETURN-CODE`.
- **Q8** — Bindings `RestApi` e `AgentAi` — aberto: implementar pelo backend ou esconder na web.
- **Q9** — Formato de fio — recomendado: um frame de texto JSON por chamada.

**Estado:** rascunho. O `CALL … RETURNING` local já está corrigido em 1.x.x e 2.x.x. A parte WEBSOCKET depende do backend, que não foi mergeado (PROGRESS PROG-79).

---
### WASM-006 — Controle WebLink

Status: rascunho. Origem: item 6 do operador. Depende de WASM-004 e WASM-007.

**Requisitos**
- **R1** — Controle **WebLink** na toolbox de projetos web e mobile, com ícone, tamanho e caption padrão. É tipo próprio que herda propriedades, eventos e métodos do Label (Q1 trata do nome e das forms desktop).
- **R2** — Mostra o **Caption** como texto de link. Cor e sublinhado (sempre, no hover ou nunca, por propriedade). Cursor **hand** por padrão.
- **R3** — Propriedades além das do Label:
  - **Url**;
  - **Target**: `Self`, `NewTab`, `NewWindow`, `Parent`, `Top` (equivalentes a `_self`, `_blank` etc.); `NewWindow` tem as features da janela (largura, altura, posição, como `window.open`);
  - **Rel**, lista (`noopener`, `noreferrer`, `nofollow`, `external`…);
  - **Download**, desligado ou com o nome de arquivo oferecido;
  - **HrefLang**; **MimeType** (atributo `type`); **ReferrerPolicy**;
  - cores dos estados **Link**, **Hover**, **Active** e **Visited** (Visited só por CSS: a página não lê se o link foi visitado);
  - **`ping` não é oferecido na v1** (operador, 2026-10-20): faz o navegador enviar requisição que a página não busca, o que o CSP de WASM-013 bloquearia, e fere a privacidade. Um spec futuro pode adicioná-lo, limitado à origem do backend.
- **R4** — Nome acessível é o Caption; uma propriedade opcional de rótulo acessível o substitui. O **Tooltip** existente vira `title`.
- **R5** — No clique, dispara primeiro o evento de clique e depois navega por `Target`. Ctrl/Cmd-clique e clique do meio abrem nova aba; Shift-clique abre nova janela; Alt-clique segue a convenção de download do navegador. O evento informa as teclas modificadoras.
- **R6** — Se o handler ativar **Cancel** durante o clique, a navegação não acontece. Cancel é lido depois que o handler retorna (Q3).
- **R7** — Esquemas aceitos: `http`, `https`, `mailto` (com `subject`, `body`, `cc`, `bcc`), `tel`, `sms`, URLs relativas e fragmentos (`#name`). **Recusa** `javascript:`, `vbscript:` e `data:` (exceto `Download` de arquivo gerado), no designer e em execução. Os esquemas aceitos são configuráveis por projeto e nunca ampliados por padrão.
- **R8** — Com `Target` `NewTab` ou `NewWindow` e `Rel` vazio, abre com `noopener noreferrer`. O desenvolvedor pode afrouxar explicitamente.
- **R9** — Recebe foco e ativa com Enter. Anel de foco visível. Na web, mantém o menu de contexto do navegador (abrir em nova aba, copiar endereço).
- **R10** — Link de fragmento que nomeia um controle ou âncora da form rola até ele e move o foco.
- **R11** — No shell mobile, endereço fora da origem abre no **navegador do sistema**, não na web view. `mailto:`, `tel:` e `sms:` vão para o sistema operacional.
- **R12** — Métodos: `SetUrl`, `GetUrl`, `Open` (clique simples, respeitando `Target`), `OpenIn(target)`, mais `SetCaption` e `GetCaption` do Label. Todos no vocabulário fechado (`methods.rs`) e no System KB.
- **R13** — Eventos do Label mais: **before-navigate** (onde Cancel vale) e **after-navigate** para navegação no mesmo contexto.
- **R14** — No Form Designer, desenhado como texto de link. Caption editável no lugar; Url no painel Properties, com validação de esquema e indicação de esquema recusado. O clique é testado na Preview (WASM-008).
- **R15** — Round-trip por `.cfrm`; teste em `tests/controls/`; exercitado pelos testes que iteram `ControlType::ALL`; documentado no System KB; ícone; strings em seis idiomas.

**Metas:** WebLink com texto de link e navegação como `<a>`; toda capacidade de `<a>` como propriedade, método ou evento, exceto `ping`; seguro por padrão.
**Fora do escopo:** rich text ou links inline dentro de um Label; navegação entre forms da aplicação (shell de WASM-011 e router proposto); prévia, verificação ou analytics de links.
**Adiado ou fora de alcance:** o controle em forms **desktop** (exigiria capacidade de runtime que não existe); `ping`; estilo `:visited` (limitado pelos navegadores).

**Questões:**
- **Q1** — Nome e alcance — aberto, com recomendação: nome WebLink e só web e mobile no 2.0.
- **Q2** — Alt-clique e modificadores variam por navegador e SO — aberto: fixar a tabela no `/plan`.
- **Q3** — Como o handler cancela — recomendado: propriedade booleana (`SET LINK-1::Cancel TO TRUE`).
- **Q4** — Navegação de mesma origem dentro de uma SPA — aberto; o router decide.
- **Q5** — Estilo de visitado restrito por privacidade — aberto: declarar o subconjunto suportado.
- **Q6** — Percent-encoding e IDN — recomendado: passar pelo parser de URL da plataforma e mostrar a forma analisada.

**Estado:** rascunho.

---

### WASM-007 — CSS3 para aparência e stylesheet padrão da form

Status: rascunho. Precedência decidida (2026-10-19): **o CSS vence**. Depende de WASM-002 e WASM-004. Necessário para WASM-006, 008 e 009.

**Requisitos**
- **R1** — **Uma tabela** mapeia cada propriedade de aparência para CSS3. Exemplos: Shadow* → `box-shadow`; gradiente → `linear-gradient`; Transparency → `opacity` ou alfa; Padding* → `padding`; CornerRadius → `border-radius`; BorderStyle/Color/Width → `border`; propriedades de fonte → `font-*` e `text-decoration`. Cada entrada é marcada *exact*, *approximate* ou *not on the web* (candidatos a approximate: perspectiva `RotateX`/`RotateY` e blur do SO). A marcação aparece no tooltip (WASM-004 R6).
- **R2** — Todo controle web ou mobile tem **`CssClass`** (nomes de classe separados por espaço) e **`Style`** (declarações inline), no modelo e no painel Properties. O nome do controle vira `id`/`data-name`, para um seletor alcançá-lo.
- **R3** — O elemento leva uma classe do tipo (ex.: `pr-button`). Pseudo-classes (`:hover`, `:focus-visible`, `:disabled`, `:checked`) ficam usáveis em seletores.
- **R4** — Toda form web ou mobile tem uma **default stylesheet**, guardada com a form e com round-trip byte a byte (comentários e formatação inclusos). Na criação, é **gerada a partir do tema**, com o CSS equivalente ao visual atual.
- **R5** — O botão de stylesheet no painel Properties da form abre uma **janela modal** com editor, coloração de sintaxe CSS e diagnósticos como warnings: erro de sintaxe, propriedade desconhecida, valor que não faz parse, recurso que o alvo não suporta. Botões: **Apply**, **Cancel**, **Revert to theme default**.
- **R6** — **Revert to theme default** pede confirmação e é desfazível. O IDE nunca descarta texto do desenvolvedor sem isso ("user code is sacred").
- **R7** — A modal tem tamanho explícito e um único grip como escritor (referência: `panels/leaderboard_modal.rs`).
- **R8** — Cascata, do mais baixo ao mais alto: (1) defaults do tema; (2) **propriedades** do painel Properties; (3) **form stylesheet**; (4) regras de `CssClass`; (5) `Style` do controle. Dentro do CSS vale a cascata comum (especificidade, ordem, `!important`). O plano pode usar uma cascade layer inferior.
- **R9** — A stylesheet vale só dentro da raiz da form e não afeta a página host (WASM-002 R12). Define CSS custom properties das cores do tema (accent, surface, texto, radius) para a página host defini-las.
- **R10** — CSS suportado: navegadores evergreen e web views das plataformas (WKWebView, WebView do Android). Inclui `@media`, container queries, `@font-face` para fontes de `Assets/` e `env(safe-area-inset-*)` no mobile.
- **R11** — Mudança em propriedade, `CssClass`, `Style` ou stylesheet aparece na Preview sem rebuild (WASM-008).
- **R12** — Override de CSS que o designer não consegue pintar (regra de classe ou `Style`) é marcado no Form Designer, sem fingir o resultado. O painel Properties marca a propriedade que uma regra CSS sobrescreve.
- **R13** — Stylesheet não carrega recurso remoto sem permissão do projeto. `url()` fica limitado a assets do projeto, data URIs e origens permitidas. Script-em-CSS é recusado. O Build imprime as fontes de estilo e de fonte que o CSP deve permitir (WASM-002 R14).
- **R14** — Form desktop não muda: não tem stylesheet, e as propriedades novas existem só em web e mobile.

**Metas:** tabela propriedade → CSS3 com marcação de exatidão; `CssClass` e `Style` por controle; stylesheet por form com editor modal; precedência declarada; preview ao vivo; stylesheet que não sai da form.
**Fora do escopo:** pré-processador, framework de utility classes, designer visual de CSS; renderizar CSS arbitrário no Form Designer egui; mudar o sistema de estilo desktop.
**Propriedades ausentes hoje:** LineHeight, LetterSpacing e FontWeight não existem, e nenhum R as adiciona.

**Questões:**
- **Q1** — Precedência — **decidido**: CSS vence o painel Properties (R8). O painel mostra quando um valor é sobrescrito (R12).
- **Q2** — Onde mora a stylesheet — recomendado: arquivo `forms/<Form>.css` ao lado da form. Alternativa: dentro do arquivo do form (`.wcfrm`/`.mcfrm`; ressalva do CDATA).
- **Q3** — Stylesheet de projeto inteiro (camada de aplicação) — recomendado: deixar para spec futuro, reservando o lugar na cascata.
- **Q4** — Nomes de classe (`pr-button`) e custom properties como contrato público — aberto: fixar no `/plan` e travar com teste.
- **Q5** — Efeitos visuais sem gêmeo CSS exato (sombras de vidro, perspectiva, cantos do spec 057) — aberto, sem recomendação: aceitar approximate por propriedade ou exigir paridade nos três temas embutidos.

**Estado:** rascunho.

---

### WASM-008 — Preview como web view

Status: rascunho. Origem: item 9 do operador. Depende de WASM-001, 002, 004 e 007. Relacionado a WASM-003 (backend de dev) e WASM-009 (login de dev).

**Requisitos**
- **R1** — A Preview de uma form web ou mobile faz o build do frontend pelo pipeline de WASM-002 (módulo, descrições das forms e player pré-compilado), sem compilar Rust, e mostra tudo numa **web view** do sistema: WKWebView no macOS, WebView2 no Windows, WebKitGTK no Linux. Não é uma aproximação desenhada pelo egui.
- **R2** — A web view vive numa janela própria. Se travar ou cair, não derruba o IDE (Q1: processo separado).
- **R3** — Mudança de form, propriedade, `CssClass`, `Style` ou stylesheet atualiza a Preview no lugar, mantendo o estado onde pode e recarregando onde não pode, dentro de um prazo fixado no plano. Mecanismo: atualização in-place do player (WASM-016 R16).
- **R4** — Controles da Preview: **presets de dispositivo** (celular, tablet, largura desktop); **orientação**; **device pixel ratio**; **safe-area insets**; esquema de cor (claro/escuro) e locale; tamanho livre por um grip. A janela **nunca se redimensiona sozinha**.
- **R5** — A web view expõe as ferramentas de desenvolvedor (inspector, console) em modo debug. Console da página e erros não capturados vão ao painel Output, com a form e, quando o engine der, a linha COBOL.
- **R6** — Se a form chama um backend (WASM-005), a Preview usa o backend do projeto em modo de desenvolvimento (WASM-003 R20) e o inicia sob pedido. Sem backend rodando, avisa em vez de falhar em silêncio.
- **R7** — Build que falha: a Preview mantém a última versão boa e mostra os erros. Os erros também vão ao Output.
- **R8** — A web view carrega só a origem de desenvolvimento do próprio projeto. Outra navegação é bloqueada ou entregue ao navegador do sistema. A página não acessa o IDE.
- **R9** — Render headless de uma form web para imagem, e execução com script (digitar, clicar, esperar, ler propriedade, devolver texto exibido, erros e imagem). Servem a testes e às ferramentas MCP `render_form` e `run_form`, com o mesmo formato das forms desktop.
- **R10** — Com projeto desktop, Preview e Run Form ficam como hoje.

**Metas:** Preview em web view a partir do mesmo pipeline do Build; atualização ao salvar e ao mudar estilo; emulação de dispositivo sem redimensionamento próprio; ferramentas de desenvolvedor e console no Output; contraparte headless para testes e agentes.
**Fora do escopo:** emuladores Android, simuladores iOS, execução em dispositivo; trocar a canvas do Form Designer por web view; depuração remota.
**Restrições:** WebKitGTK e WebView2 viram dependências de runtime do IDE; instaladores e checagem de toolchain precisam dizê-lo; licenças do crate de web view; i18n de presets e erros.

**Questões:**
- **Q1** — Processo separado ou embutido — recomendado e **decidido no plano**: processo e janela separados, como o Run Form.
- **Q2** — Biblioteca de engine — **decidido no plano**: wry 0.57 + tao 0.35.
- **Q3** — Backend durante a Preview — recomendado: backend real iniciado localmente. O plano diz que o IDE não inicia o Backend e a Preview avisa.
- **Q4** — Login durante a Preview — aberto: provedor real ou token de desenvolvimento (WASM-003 Q5).
- **Q5** — Orçamento de atualização (R3, "N ms") — aberto: medir um protótipo antes de fixar.
- **Q6** — A Preview substitui o Run Form nos tipos web? — **decidido no plano**: não. Duas ações: **Preview** (web view com controles de dispositivo), **Run** (mesmo servidor no navegador do sistema), **Build** (bundle e relatório).

**Plano (plan.md):** status "implementado em `worktree-agent-a97ba65c…`" com seis entregas. Abordagem em três passos: build → serve → show. Servidor de dev em loopback (`std::net`, sem dependência nova; checagem do cabeçalho Host, caminho canonicalizado, só GET/HEAD). Helper `prc-web-preview` (wry + tao, processo próprio; testado em macOS). Watcher de forms, `src`, `Assets`, backend e manifesto com rebuild e `notify_changed`. Presets: 7 presets × 2 orientações, pixel ratio 1/1.5/2/3 e safe-area que gira com a tela. Protocolo JSON fixado: IDE→helper `open`, `size`, `reload`, `scheme`, `devtools`, `close`, `emulate`; helper→IDE `ready`, `console`, `error`, `blocked`, `failed`, `closed`. Política de navegação restrita à origem da Preview.

**Tarefas (tasks.md):** T1–T10 feitas, T11–T13 abertas. **Abertas:** T11 abrir uma janela real uma vez (operador); T12 ligar o gerador real e medir AC1–AC2; T13 backend iniciado sob pedido (R6) e render headless/execução de form web (R9, depende de WASM-015 R1–R2). Não terminado segundo o plano: R3 é reload, não atualização in-place; erros não são pintados sobre a página; `@media (resolution)` e `env(safe-area-inset-*)` não mudam com a emulação; Windows e Linux não testados.

---

### WASM-009 — Controle Authentication e backend de provedores de identidade

Status: rascunho. O fluxo é escolhido por provedor (2026-10-19); as capacidades de cada provedor ainda precisam ser verificadas. Depende de WASM-003, 005 e 007. Relacionado ao controle `AwsCognito`.

**Requisitos**
- **R1** — Controle **Authentication** na toolbox de web e mobile, com as telas: **Sign in**, **Create account**, **Confirm account** (código por e-mail ou telefone), **Forgot password** (pedido, depois código e nova senha), **MFA challenge** (opcional) e a ação **Sign out**. O desenvolvedor escolhe quais telas existem; os textos são traduzíveis.
- **R2** — Estilizado como qualquer controle (propriedades, `CssClass`, `Style`, stylesheet). Define `autocomplete` correto (`username`, `current-password`, `new-password`, `one-time-code`) para gerenciadores de senha. Operável por teclado e leitor de tela.
- **R3** — Exposto ao COBOL:
  - eventos **onSignedIn**, **onSignedOut**, **onSignUpCompleted**, **onPasswordResetRequested**, **onMfaRequired**, **onError**;
  - propriedades **State**, **UserName** e **Email** do usuário, e **Mode**;
  - métodos **SignOut**, **Refresh**, **GetClaim**.
  Os nomes são fixados no `/plan` e registrados no vocabulário fechado e no System KB.
- **R4** — (Opcional) **reCAPTCHA v3**: o controle obtém token por ação protegida (sign-in, sign-up, forgot-password), sem desafio visível, e o envia. O backend verifica com o provedor usando segredo que só existe no backend, checa nome da ação e hostname, e recusa score abaixo do limiar (padrão 0.5) com a mesma mensagem genérica de senha errada. Site key é pública; secret nunca no frontend.
- **R5** — **Access token** só em memória (nunca `localStorage` ou `sessionStorage`). **Refresh** não é legível por script: cookie `HttpOnly`, `Secure`, `SameSite`, posto pelo backend na web; armazenamento seguro do SO no mobile. Renovação silenciosa antes de expirar. Se o refresh falha, dispara **onSignedOut**.
- **R6** — Tokens nunca vão para log, URL ou texto visível. `GetClaim` devolve o valor de uma claim, nunca o token.
- **R7** — Backend de identidade = conjunto de **programas públicos** gerados do Backend (WASM-003 R9, R22): sign-in, sign-up, confirm, forgot-password, reset-password, MFA challenge/verify, refresh e sign-out.
- **R8** — Suporte a provedores por **interface de adaptador**: um quarto provedor é um adaptador, não mudança no controle. Três adaptadores (AWS Cognito, Keycloak, Microsoft Entra), cada um com: autenticação, registro, confirmação, recuperação de senha, desafio MFA, refresh, sign-out/revogação e atributos básicos, dentro do que cada provedor oferece. Uma tabela no plano lista o que não é suportado e o que o controle faz no lugar.
- **R8b** — **Fluxo escolhido por provedor** (operador, 2026-10-19). Cada adaptador declara **in-app** (telas do controle, backend troca credenciais com o provedor) ou **hospedado** (navegador ou web view vai à página de login do provedor, intermediado pelo backend com authorization code e PKCE). Os eventos de R3 são os mesmos nos dois fluxos.
- **R9** — Configurações do provedor na configuração do Backend, por uma **página de settings do IDE** com ação **Test connection**: issuer, ou region/realm/tenant; client id; client secret eventual; segredo do reCAPTCHA. Nunca em form, copybook ou arquivo do frontend.
- **R10** — Falhas não revelam se a conta existe (mesma resposta para usuário desconhecido e senha errada, e para reset com endereço conhecido ou não). Rate limit de sign-in, sign-up e reset por origem e por conta, somado ao lockout do provedor. Senhas nunca vão para log.
- **R11** — Enquanto a aplicação exige login para uma form (Q4), mostra a form Authentication a quem não entrou, e a form pedida depois do login.
- **R12** — Testes automatizados rodam os fluxos completos contra um **provedor local** (Keycloak em contêiner e mock para os demais). Testes contra tenants reais de Cognito e Entra são do operador, e a aceitação registra quais rodaram.

**Metas:** controle Authentication com as telas; reCAPTCHA v3 verificado no backend; backend neutro com adaptadores para Cognito, Keycloak e Entra; tokens tratados para o script da página não roubar a credencial de longa duração; eventos e propriedades úteis à form.
**Fora do escopo:** login social e federação (Google, Apple); passkeys/WebAuthn; autorização por papéis; console de administração; substituir o controle `AwsCognito`; hospedar um provedor. Cada um é proposto em WASM-031.
**Adiado:** token de sessão emitido pelo backend; hCaptcha e Cloudflare Turnstile (adaptadores posteriores); "Sign in with Apple" no iOS.

**Questões:**
- **Q1** — Risco da senha digitada na nossa form — **decidido**: fluxo por provedor (R8b). Para `/clarify`: capacidades de cada produto e fluxo padrão de cada adaptador. Recomendado: Cognito in-app; Entra e Keycloak hospedados, salvo tenant configurado para in-app.
- **Q2** — Token do provedor ou do backend — recomendado: o do provedor, validado pelo backend (WASM-003 R10). Token do backend é opção posterior.
- **Q3** — Cookie de refresh exige HTTP, e a série usa WebSocket (WASM-003 Q3) — aberto: confirmar a pequena superfície HTTPS.
- **Q4** — Proteção de rota — recomendado: propriedade "requires sign-in" no nível da form.
- **Q5** — Tipos de MFA no 2.0 — aberto: só TOTP ou também SMS e e-mail.
- **Q6** — reCAPTCHA v3 e privacidade — aberto: o controle oferece um consent hook?
- **Q7** — Mobile: armazenamento seguro e biometria — aberto, a confirmar com o spec de plataforma mobile (WASM-012).

**Estado:** rascunho.

---

### WASM-010 — Geração de código WebAssembly

Status: rascunho. Resolve WASM-002 Q1. Necessário para WASM-004, 005, 008 e 015. Depende de WASM-002.

**Requisitos**
- **R1** — Build de projeto Web ou Mobile emite **um módulo WebAssembly para o projeto inteiro** a partir do COBOL do lado do navegador: forms geradas, handlers de evento, procedures e o Common Code que elas `CALL`. Cada form leva uma cópia do Common Code que chama (WASM-017 R1). Há variante multi-memory e single-memory (WASM-017 R9). Emissão direta da AST, sem compilar Rust e sem interpretador, AST ou pacote de runtime na saída.
- **R2** — Módulo **autocontido**: importa só a host API (R12) e carrega só as rotinas de que precisa. Exports: os pontos de entrada de R10, os de R9 e a memória.
- **R3** — Todo item de `WORKING-STORAGE`, `LOCAL-STORAGE` e `LINKAGE` mora num lugar fixo da memória linear, com **layout byte a byte**: `PICTURE` e `USAGE` (`DISPLAY`, `COMP`/`COMP-5`, `COMP-3`, `COMP-1`/`COMP-2`, national); posição do sinal (overpunch final, `SEPARATE`, `LEADING`); `OCCURS` inclusive `DEPENDING ON`; grupos como bytes contíguos; `REDEFINES` com os mesmos bytes; `FILLER`, `VALUE` e `INITIALIZE`. O tamanho é **fixo em tempo de compilação**: o código nunca chama `memory.grow`. Cada instância de form tem região própria (WASM-017 R2). A **transport arena** é a memória 1 (multi-memory) ou uma região reservada da memória 0 (single-memory).
- **R4** — Aritmética **decimal exata de ponto fixo**: no mínimo 18 dígitos do padrão e até a largura que o interpretador suporta (mantissa `i128`, cerca de 38 dígitos). `ADD`, `SUBTRACT`, `MULTIPLY`, `DIVIDE` e `COMPUTE`, com `GIVING`, `REMAINDER`, todo modo `ROUNDED` e `SIZE ERROR`, com os resultados do interpretador (R16).
- **R5** — Controle de fluxo completo: parágrafos e seções; `PERFORM` em todas as formas (simples, `THRU`, `TIMES`, `UNTIL`, `VARYING`, inline, `TEST BEFORE`/`AFTER`, `EXIT PERFORM`/`CYCLE`); `GO TO` (com `DEPENDING`) e **`ALTER`**; `EVALUATE`, `IF` e `NEXT SENTENCE`; programas aninhados; `CALL`, `CANCEL`, `GOBACK`, `EXIT PROGRAM` e `STOP RUN`. Nenhum programa que compila é recusado por usar `GO TO` ou `ALTER`.
- **R6** — Rotinas emitidas no módulo: aritmética decimal, incluindo operações de 128 bits que o WebAssembly não tem; numeric-edited; strings para `STRING`, `UNSTRING`, `INSPECT` e reference modification; funções intrínsecas; aritmética de data e hora. Devem ter **uma fonte de verdade com o interpretador** (Q3). Sem código de arquivo, thread, processo, `redb` ou `std::fs`.
- **R7** — Subconjunto: todo statement COBOL-85 que não precisa de arquivo, mais as extensões PowerRustCOBOL que não precisam da máquina host: propriedade de controle lida e escrita como item de dados (`CTRL::Property`); chamadas de método e valores de método; value methods de itens (`Trim`, `UpperCase`, `Replace`, `Len`…); expressões em `MOVE`/`SET`/`STRING`; built-ins inline `COBOL::"…"` que servem ao navegador. Cada built-in de `BUILTINS` é classificado como *browser*, *backend only* ou *unsupported* numa tabela.
- **R8** — Construção fora do subconjunto: erro na linha, nomeando a construção e a alternativa (WASM-002 R3). Teste falha quando uma variante de `Stmt` ou built-in não tem entrada na tabela.
- **R9** — **Suspensão**: enquanto uma chamada espera (CALL remoto, diálogo, espera temporizada), o módulo para, devolve o controle e retoma no mesmo ponto. Posição, pilha de `PERFORM` e dados ficam na memória linear (Q4). Um handler que estoura um **orçamento** de tempo cede ao host num back-edge e retoma no próximo turno. Com a página isolada, o módulo roda num Worker (WASM-017 R19). O resultado não depende de onde cedeu.
- **R10** — Evento do host (id do evento, id do controle, parâmetros): o módulo roda o handler ligado até o fim ou a próxima suspensão, aplicando ao host o que muda, e avisa quando termina.
- **R11** — Erro de execução (subscrito fora da tabela, divisão por zero sem `SIZE ERROR`, dado numérico inválido, `CALL` não resolvido): `ON SIZE ERROR` e `ON EXCEPTION` se comportam como no interpretador. Erro não tratado encerra o handler, é reportado com a linha COBOL (R14) e a aplicação continua.
- **R12** — **Host API** curta e versionada, cada entrada com significado documentado: ler e definir propriedade de controle; chamar método de controle; escrever `DISPLAY` no console; ler relógio e número aleatório; fazer chamada remota. A página a implementa (WASM-002). Módulo para a API *n* se recusa a iniciar num host que não a ofereça. A API é **de granularidade grossa**: escritas de propriedade de um handler são agrupadas e entregues numa chamada ao fim ou à suspensão, com leituras refletindo escritas anteriores (WASM-017 R11, R12).
- **R13** — Fora as chamadas ao host, o módulo é **determinístico**: mesmos eventos e respostas dão mesmos resultados. O harness de conformidade depende disso.
- **R14** — **Name section e line table** mapeiam instruções para linhas COBOL, com form e evento. Suficiente para as mensagens de R11 e para o debugger proposto (WASM-024).
- **R15** — O Build reporta tamanho do módulo e tempo de startup. Nenhum limite é fixado aqui (Q6).
- **R16** — **Paridade medida** entre módulo e interpretador:
  - (a) programas NIST CCVS85 dos módulos **NC**, **IF** e **IC** no subconjunto compilam e rodam num engine WebAssembly headless com **0 falhas** nos relatórios CCVS; fora do subconjunto, cada programa é listado com a construção que o exclui;
  - (b) programas de `tests/cobol/` no subconjunto imprimem a mesma saída nos dois engines;
  - (c) onde diferem, um lado tem bug: corrigido na 1.x.x se for o interpretador (e portado), e aqui se for o gerador. Nunca "documentar a diferença para sumir".
- **R17** — A regra de paridade interpretador/binário lista o gerador e as rotinas emitidas como mais um consumidor, e nomeia as rotinas que são fonte compartilhada.
- **R18** — **Fases**, cada uma utilizável: **A** — layout, `MOVE`, aritmética, `IF`, `PERFORM`, `GO TO`, `EVALUATE`, host API para propriedades, métodos e eventos; **B** — tabelas, `STRING`/`UNSTRING`/`INSPECT`, numeric-edited, funções intrínsecas, programas aninhados e `CALL` local; **C** — suspensão e `CALL` remoto com mapeamento JSON (WASM-005); **D** — o resto do subconjunto. Cada fase registra o que compila e o que passa, no estilo do ledger NIST. Pela GOLDEN RULE #9, NC termina antes de IF e IC.

**Metas:** back end que transforma o COBOL do browser num único módulo autocontido; layout byte-exato e aritmética exata com os resultados do interpretador; todo controle de fluxo COBOL-85, inclusive `GO TO` e `ALTER`; módulo que pausa e retoma; host API curta como único import; **gate de conformidade**: NIST do subconjunto e corpus dourado com 0 falhas.

**Fora do escopo:** E/S de arquivo de qualquer organização, `redb`, `std::fs`, processos filhos, sockets, SQL, `EXEC SQL` e `EXEC RUST` (no browser os dados moram no backend); threads dentro da semântica COBOL; runtime, interpretador ou AST no módulo; compilar o backend; GC, component model, threads ou SIMD na v1; otimização além de uma redução direta.

**Questões:**
- **Q1** — "Diretamente" — **decidido e confirmado** (operador, 2026-10-20): manter a geração direta, contra o transpilador COBOL → Rust e o interpretador compilado para `wasm32`.
- **Q2** — Emitter e engine de teste — aberto: `wasm-encoder` ou escritor próprio; engine `wasmi` ou `wasmtime`. Sondar licença, MSRV e peso no `/plan`.
- **Q3** — Como as rotinas são escritas — recomendado: opção (a), escritas uma vez em Rust, compiladas na construção do produto, guardadas como blob e ligadas estaticamente em cada módulo com as funções não usadas descartadas. Uma fonte de verdade com o interpretador.
- **Q4** — Técnica de suspensão — recomendado: estado explícito mantido pelo código gerado (funciona em todo navegador). Alternativas rejeitadas: JSPI e Asyncify.
- **Q5** — Paridade do modelo de dados (bytes reais × valores) — aberto, a confirmar. Diferença achada é bug do interpretador (R16c).
- **Q6** — Orçamentos de tamanho e startup — aberto: medir a fase A antes.
- **Q7** — Quais built-ins servem ao navegador — aberto, decidido entrada por entrada no `/plan`.
- **Q8** — Arquivos indexados e `SORT` em memória no browser — aberto, a confirmar; fora da v1.
- **Q9** — `DISPLAY` e `ACCEPT` — aberto: `DISPLAY` vai ao console do host; `ACCEPT` é recusado ou mapeado para diálogo?
- **Q10** — Um módulo ou dois — **decidido** (2026-10-19): dois `.wasm` (módulo e player, WASM-016).

**Estado:** rascunho. Cobertura NIST citada no estado atual: NC, IF, IC, SM, ST, SQ, IX e RL terminados; DB só compila; SG fora de escopo (para o interpretador). Para o gerador, PROGRESS registra NC, IF e IC terminados (seção 7).

---
### WASM-011 — Navegação e estilos de aplicação: SPA por padrão, forms standalone como alternativa

Status: rascunho. Depende de WASM-001, 002 e 010. Parte da spec 049 (shell, `FormFormat`, `super`) e da spec 037 (ciclo de vida).

**Requisitos**

*Estilo da aplicação*
- **R1** — Estilo é configuração do projeto, escolhida no New Project (WASM-001 R4) e alterável antes do Build. Valores: **SPA** (padrão) ou **Standalone**.
- **R2** — Comportamento por estilo. **SPA:** `OpenFormSync` é **diálogo modal** dentro da página, com o chamador suspenso até fechar (WASM-010 R9); `OpenFormAsync` é **painel modeless** dentro da página; carga por menu (shell) carrega no ContentPane (spec 049). **Standalone:** `OpenFormSync` abre nova aba ou janela (em mobile, tela empilhada), chamador suspenso; `OpenFormAsync` abre nova aba ou janela, chamador continua; carga por menu não é usada (não há shell).
- **R3** — `FormFormat` (`Standalone`, `Embedded`, `Both`, spec 049) mantém o significado e a checagem de build: form `Embedded` não pode ser alvo de abertura standalone, e form `Standalone` não pode ser carregado num pane. Em SPA, form `Standalone` ou `Both` aberto por `OpenFormSync`/`OpenFormAsync` aparece como diálogo ou painel in-page; não é carga no ContentPane.

*SPA*
- **R4** — Ao mostrar um form, o endereço vira a **route** do form (nome mais parâmetros de abertura). Back e Forward percorrem a navegação. Reload ou endereço colado abre a aplicação naquele form (o que é restaurado depende de Q1). A **data region** do form é criada com ele e liberada na destruição, não enquanto está residente ou preservado por `PreservePreviousForm` (WASM-017 R2, R5). O módulo é carregado uma vez, no start.
- **R5** — A route funciona em qualquer host estático sem configuração de servidor (Q2), e não sequestra o endereço da página quando a aplicação está embutida (WASM-002 R11).
- **R6** — Endereço sem form, ou com form que o usuário não pode abrir: a aplicação mostra desfecho definido de **not-found** ou **sign-in** (WASM-009 R11) e dispara um evento que o desenvolvedor pode tratar.
- **R7** — A cada navegação: foco no título do novo form, anúncio à tecnologia assistiva, scroll reiniciado.
- **R8** — (Opcional) Form com alterações não salvas: sair por Back, menu ou fechando a aba pede confirmação, dentro do que o navegador permite.

*Standalone*
- **R9** — Cada form standalone tem **endereço e página próprios**, abríveis diretamente (bookmark, link), sujeitos a sign-in, e roda em **instância própria**, com dados próprios, como no desktop.
- **R10** — Abertura standalone a partir de gesto do usuário abre a janela. Se o bloqueador de popup recusa (chamada por timer ou após espera), a aplicação diz como permitir, dispara evento, e o chamador suspenso é liberado com falha.
- **R11** — `super` num standalone é o form que o abriu: ler e gravar propriedades e chamar métodos por mensagens entre janelas de mesma origem. Assíncrono para a plataforma, síncrono para o programa. Falha com `ON EXCEPTION` se o opener foi fechado (Q3).
- **R12** — Ao fechar uma janela standalone, os eventos de ciclo de vida (spec 037) rodam até onde o navegador permitir, e um chamador suspenso em `OpenFormSync` é liberado.

*Mobile*
- **R13** — No shell mobile (WASM-012) não há abas nem janelas: standalone são **telas empilhadas** numa navigation stack. Gesto ou botão Back do sistema desempilha; deep link abre um form como em R4 e R9.

*Geral*
- **R14** — Diálogos modais prendem o foco, fecham pelo teclado quando o form permite, empilham na ordem de abertura e bloqueiam o que está atrás.
- **R15** — Projeto Desktop permanece inalterado.

**Metas:** estilo em nível de projeto, SPA padrão; comportamento definido para cada combinação de estilo × verbo na web e no mobile; URLs, Back/Forward e reload para SPA, mais deep links; `super`, eventos de ciclo de vida e `FormFormat` com o significado de hoje.
**Fora do escopo:** compartilhamento de dados entre forms (run unit compartilhada, `EXTERNAL`, `USING` em `OpenFormSync`), que fica para spec própria como na 049; router no servidor ou SSR; animações entre rotas; mudar o modelo de janelas do desktop.
**Previsto pelo spec:** novos eventos (not-found, popup-blocked, leave-confirmation); textos de i18n em seis idiomas; routes e páginas de entrada geradas a partir da lista de forms, nunca editadas à mão; janela de popup usa o tamanho de projeto do form.

**Questões:**
- **Q1** — O que reload ou deep link restaura — recomendado (A): o form e seus parâmetros, não os dados. Opção B: snapshot dos dados em `sessionStorage` (nunca tokens, WASM-009 R5).
- **Q2** — Sintaxe da route — recomendado: hash `#/customer/42` (funciona em qualquer host estático). Alternativa: path `/customer/42` (exige servidor que sirva a aplicação para paths desconhecidos).
- **Q3** — `super` entre janelas — aberto: o canal só existe na mesma origem e no mesmo perfil. Confirmar o limite e que opener fechado é `ON EXCEPTION` normal.
- **Q4** — Navegação multi-página na mesma janela (cada `OpenForm` substitui a página) — aberto: fora de escopo, ou parte de "standalone"?
- **Q5** — Estilo padrão no mobile — aberto, com recomendação: SPA com pilha.
- **Q6** — Módulo de programa de uma página standalone — **decidido** (2026-10-20, WASM-017 Q1): toda página carrega o módulo único e o player único, instanciados com o form inicial; o navegador faz cache de ambos.
- **Q7** — Várias instâncias da aplicação na mesma página — aberto: permitido ou recusado.

**Estado:** rascunho.

---

### WASM-012 — Shell mobile (Tauri 2): builds Android e iOS, dispositivos, assinatura e permissões

Status: rascunho. Depende de WASM-001, 002 e 010. Relacionado a WASM-009 e WASM-013. O próprio spec declara que as afirmações sobre Tauri são de conhecimento geral e devem ser verificadas no `/plan`.

**Requisitos**

*Build e toolchain*
- **R1** — Build de projeto Mobile produz, para cada plataforma escolhida, um pacote Android (instalável e formato de loja) e um archive iOS. Ambos embrulham o bundle de WASM-002 no shell Tauri, com a identidade do projeto (R6).
- **R2** — Ferramenta ausente (Android SDK, NDK ou JDK; targets Rust de Android ou iOS; Xcode e command-line tools; Tauri CLI na versão fixada): o IDE diz qual, para que serve e como obter. Instala só o seguro (`rustup target add`), nunca Xcode nem Android SDK. Plataforma que a máquina não constrói (iOS fora de macOS) aparece como indisponível com o motivo.
- **R3** — Rodar em dispositivo: lista emuladores, simuladores e dispositivos conectados; inicia o escolhido; instala e lança; recarrega o frontend a cada mudança; mostra o log do dispositivo no painel Output; para sob pedido.

*Identidade e assinatura*
- **R4** — Assinatura de release com credenciais do próprio desenvolvedor (keystore Android; certificado e provisioning profile Apple), referenciadas por nome. **Senhas e chaves ficam no secret store do SO e nunca em arquivo do projeto.** Sem o store disponível, o IDE se recusa a continuar, em vez de cair para plaintext. Builds de debug assinam automaticamente.
- **R5** — Build recusa pacote de release não assinado ou assinado com chave de debug, e informa.
- **R6** — Identidade do projeto: application id em DNS reverso; nome (traduzível nos seis idiomas); versão e build number derivados da versão do projeto; ícones e splash screen. Ícones gerados em todos os tamanhos exigidos a partir de uma imagem em `Assets/`.

*Permissões e recursos de dispositivo*
- **R7** — Cada recurso de dispositivo usado é **declarado** pelo programa (capability que o compilador acha nas chamadas e controles). O Build escreve as permissões da plataforma (Android, iOS usage descriptions) e falha, nomeando recurso e linha, se o programa usa recurso não declarado.
- **R8** — Na primeira necessidade, a aplicação pede a permissão naquele momento, com o texto de justificativa do desenvolvedor (traduzível). A recusa chega ao programa como falha normal que ele pode tratar.
- **R9** — Recursos de dispositivo usáveis em COBOL: conjunto **curto e versionado**, cada um uma entrada de host API (WASM-010 R12) com permissão, significado e entrada na KB. Primeiro conjunto (Q1): **secure storage** (WASM-009 R5), **abrir endereço** no navegador ou discador (WASM-006 R11) e **deep links**. Depois, um de cada vez: câmera, localização, notificações, biometria, haptics, share sheet, clipboard, leitor de código de barras.

*Comportamento no dispositivo*
- **R10** — O shell passa à página as **safe areas** e o tamanho do teclado como variáveis CSS (WASM-007 R10), respeita a orientação fixada e entrega o gesto Back à navegação de WASM-011 R13.
- **R11** — Background, retorno e encerramento disparam os eventos de ciclo de vida (spec 037) até onde a plataforma permitir. Ao voltar, reestabelece a conexão com o backend (WASM-003 R8) e, se a sessão expirou, pede novo sign-in.
- **R12** — A página vem do bundle, nunca da rede. Só o frontend empacotado chama comando nativo (capability list com os comandos permitidos). Endereços fora da origem abrem no navegador do sistema. A web view roda sob a CSP de WASM-013.
- **R13** — O Build imprime checklist do que as lojas vão pedir (declarações de privacidade iOS, data-safety Android), derivado dos recursos declarados.
- **R14** — Projeto Desktop ou Web não muda. As opções do tipo Mobile no New Project (orientação padrão, OS mínimo) são honradas.

**Metas:** pacote Android e iOS do mesmo frontend pela IDE; detecção honesta de toolchain com o próximo passo exato; rodar em emulador, simulador ou dispositivo com logs na IDE; assinatura com credenciais do desenvolvedor fora dos arquivos; permissões declaradas e pedidas na hora certa; conjunto pequeno de recursos de dispositivo.
**Fora do escopo:** watch e TV (WASM-034); submissão às lojas (manual); build iOS em Windows ou Linux; auto-atualização fora das lojas; UI nativa (as telas são as do web player).
**Previsto pelo spec:** o CLI Tauri tem subcomandos `android` e `ios` (`init`, `dev`, `build`); o IDE já guarda segredos no store do SO com fallback plaintext, e o caso mobile não herda esse fallback; o shell é artefato gerado, nunca editado à mão; **um build Rust por aplicação é inevitável no mobile**, ao contrário do Web; dependências Tauri passam pelos dependency guards (sem `aws-lc-rs` e `ring`, MSRV 1.92).

**Questões:**
- **Q1** — Primeiros recursos de dispositivo — aberto: decidir entre câmera, localização, notificações, biometria e leitor de código de barras para o 2.0.
- **Q2** — Build em nuvem ou remoto para iOS — aberto: fora de escopo, ou spec posterior.
- **Q3** — Níveis mínimos de OS (API Android, versão iOS) — aberto.
- **Q4** — Tablets — recomendado: Android Tablet e iPad são templates separados, diferindo dos phones só pelos breakpoints. Alternativa: shell separado.
- **Q5** — Pinning de Tauri e plugins — aberto: fixado por projeto com caminho de upgrade, ou acompanhando o IDE.
- **Q6** — Certificate pinning e hardening da conexão com o backend — aberto.

**Estado:** rascunho. Não iniciado segundo PROGRESS (PROG-82).

---

### WASM-013 — Arquitetura de segurança e threat model da linha WebAssembly

Status: rascunho. Depende de WASM-002, 003, 005, 009, 010 e 012. Dono das regras que estes citam (WASM-002 R7, R14; WASM-003 R14–R17; WASM-005 R14, R17; WASM-006 R7–R8; WASM-007 R13; WASM-008 R8).

**Requisitos**

*O modelo*
- **R1** — **Threat model** na série: ativos (contas, tokens, dados, segredos, código-fonte do desenvolvedor); atores (anônimo, autenticado, página maliciosa que embute ou abre a aplicação, cliente de backend malicioso, dependência maliciosa, arquivo de projeto malicioso aberto no IDE); **trust boundaries** (browser ⇄ backend, host page ⇄ módulo, módulo ⇄ player, IDE ⇄ web view de preview, shell mobile ⇄ frontend, backend ⇄ provedor e dados). Para cada ameaça: controle e teste que a prova. Revisado no `/analyze` de cada fase.

*Políticas que o build gera*
- **R2** — Host page com **CSP default-deny**: scripts da origem e `'wasm-unsafe-eval'`, sem `'unsafe-eval'` e sem `'unsafe-inline'` (nonce ou hash onde inevitável); estilos da origem e hashes; imagens e fontes da origem; **`connect-src` limitado aos endereços de backend**; `base-uri` e `form-action` restritos; `frame-ancestors` pelo modo de embutimento. **Trusted Types** imposto onde há suporte.
- **R3** — Hosts estáticos podem não deixar definir headers. O Build entrega a política como `<meta>` (o que a meta expressa) e como conjunto de headers pronto para hosts comuns (o que a meta não expressa: `frame-ancestors`, report-only, HSTS), e imprime qual é qual.
- **R4** — O backend envia `Strict-Transport-Security`, `X-Content-Type-Options`, `Referrer-Policy` restritivo e `Permissions-Policy`; a configuração TLS recusa versões e ciphers abaixo de um piso declarado.

*XSS*
- **R5** — Todo valor que chega à página (caption, item, célula, tooltip, mensagem) é inserido como **texto**, nunca parseado como markup. O player não tem nenhuma API que insira string HTML (`innerHTML`, `outerHTML`, `insertAdjacentHTML`, `document.write`), e um teste busca esses sinks no código. O único controle que renderiza markup, o **subconjunto HTML do Viewer**, constrói nós DOM (`createElement`, `setAttribute`, `textContent`) a partir de árvore parseada e filtrada por **allow-list** de elementos, atributos e esquemas de URL.

*CSRF, CORS e origem*
- **R6** — Chamadas que mudam estado são autorizadas por **bearer token adicionado pela página**, nunca só por cookie. Requisição que depende de cookie (refresh de WASM-009 R5) usa `SameSite`, token CSRF ou header que o navegador não adiciona cross-site. A **Origin** do WebSocket é verificada (WASM-003 R14). Superfície HTTPS com allow-list explícita, **nunca wildcard com credenciais**.

*Segredos*
- **R7** — **Secret scan** que barra Build web ou mobile (WASM-002 R7), com padrões deste spec: chaves e tokens de nuvem, blocos de private key, JSON Web Tokens, connection strings, formatos de API key, strings de alta entropia acima de comprimento configurável. Roda sobre bundle, forms, copybooks e stylesheets. Falha informando arquivo e padrão, nunca o valor.
- **R8** — Chave de backend ou de assinatura nunca cai para plaintext: sem o store do SO, o produto para e diz (WASM-012 R4).

*Supply chain*
- **R9** — Todo crate de frontend, player, backend e shell passa por **allow-list** guardada por dependency-guard (estendido da spec 063 AC8 / 103), com lock file, auditoria de vulnerabilidades, checagem de licença e **nenhum build script que busque da rede**, salvo o declarado. JavaScript da host page é escrito à mão e pequeno (WASM-002 Q6), sem árvore de gerenciador de pacotes.
- **R10** — Cada Build escreve um **SBOM** em formato padrão, e os assets da host page carregam hashes de integridade.

*Seguro por padrão*
- **R11** — Toda relaxação (token de desenvolvimento, `ws://` para endereço não-loopback, origem wildcard, CSP enfraquecida, pular o secret scan) é configuração explícita cujo nome diz **unsafe**, mostra aviso no IDE e **faz o Build de release falhar**.
- **R12** — Backend de desenvolvimento faz bind em loopback salvo instrução, e a web view de preview carrega só sua origem (WASM-008 R8).

*Logging e privacidade*
- **R13** — Logs, crash reports e telemetria do backend, do frontend e do IDE nunca contêm token, senha, segredo ou campo marcado como sensível. O plano lista os dados pessoais tratados e para onde vão.

*Mobile*
- **R14** — A capability list do shell dá a cada janela o mínimo de acesso nativo, nenhum à origem remota, e é gerada dos recursos declarados (WASM-012 R7, R12).

*Garantia*
- **R15** — A suíte inclui: auditoria de dependências e licenças; secret scan contra segredos plantados; teste de que CSP e headers gerados são iguais ao texto esperado, e de que uma página sob eles não executa script inline nem `eval`; busca de sinks XSS; **fuzzing** do tratamento de frames WebSocket, do mapeamento JSON (WASM-005) e da validação de token (WASM-003 R10), com orçamento de execução declarado.
- **R15b** — (constraint) A aplicação **usa cross-origin isolation e threads quando a página os oferece, e funciona sem eles** (operador, 2026-10-20; WASM-017 R19). Com isolamento (`COOP: same-origin`, `COEP: require-corp` ou `credentialless`), a página ganha `SharedArrayBuffer`. Custo: todo recurso cross-origin precisa enviar CORS ou CORP, ou carregar sob `credentialless`. O caminho plain é fallback testado como caminho completo. Memória do módulo nunca cresce, a arena é fixa, a shared memory tem máximo declarado, e o loader libera por completo regiões, módulo e Workers de forms e aplicações destruídas (WASM-017 R2, R5, R7, R8).
- **R16** — Quando a série chega a release candidate, **revisão por alguém que não seja o autor** e, se o operador decidir (Q1), pentest externo, são registrados antes do lançamento.

**Metas:** threat model com trust boundaries nomeadas e controle por ameaça; CSP estrita e conjunto de headers gerado e testado; regras de XSS, CSRF, CORS, tokens, segredos, supply chain e logging, cada uma testável; defaults seguros com relaxação explícita e recusada em release; checagens automatizadas (dependências, secret scan, política, sinks, fuzzing).
**Fora do escopo:** hardening do SO, anti-tampering, detecção de root ou jailbreak; WAF; internals de identity provider; certificação de conformidade (proposta em WASM-030).

**Questões:**
- **Q1** — Pentest externo — aberto: obrigatório antes do 2.0, ou revisão por segundo leitor; orçamento e fornecedor são do operador.
- **Q2** — Trusted Types — aberto: impostos só onde há suporte, e exigem que o código do player cumpra.
- **Q3** — Rigor do secret scan — recomendado: entropia alta como **aviso**; formatos conhecidos como **erro**.
- **Q4** — Contato de segurança e política de divulgação (arquivo `SECURITY`) — aberto; necessário antes do 2.0.
- **Q5** — Certificate pinning para o backend do shell mobile — aberto: opção, exigência ou escolha do desenvolvedor.

**Estado:** rascunho. Não iniciado segundo PROGRESS (PROG-83).

---

### WASM-014 — Contrato do backend e sua evolução

Status: rascunho. Depende de WASM-003 e WASM-005. Relacionado a WASM-009, 012 e 013.

**Requisitos**

*Versões e handshake*
- **R1** — Todo build de backend tem uma **contract version** e uma versão de wire protocol. Todo build de frontend carrega a contract version contra a qual foi compilado. Ambas ficam na interface description (WASM-003 R4).
- **R2** — Ao abrir a conexão, os lados trocam versões e o backend responde **accepted**, **accepted with a warning** (cliente mais velho, compatível) ou **refused**, com motivo e, quando possível, a versão a atualizar. O frontend dispara evento definido; em recusa, a aplicação pode pedir atualização (link da loja no mobile).
- **R3** — A versão do protocolo permite servir mais de uma versão ao mesmo tempo, dentro de uma janela declarada. Cliente fora da janela é recusado com mensagem de atualização, nunca com erro de protocolo.

*Compatibilidade*
- **R4** — Regras de mudança **compatível** e **incompatível** num exported program, escritas e checadas pelo build. Compatível: campo final opcional, novo exported program, `OCCURS` ampliado, nova classe de erro. Incompatível: remover ou renomear campo, mudar `PICTURE` ou `USAGE` de forma incompatível, estreitar `OCCURS`, mudar roles exigidos, remover programa.
- **R5** — No build do backend, a nova interface description é comparada com a **última liberada** (guardada no projeto). Cada mudança incompatível é reportada com programa, item e regra. Release Build falha, salvo se o desenvolvedor subir deliberadamente a versão **major** do contrato.
- **R6** — Programa pode ser marcado **deprecated**, com mensagem e versão de remoção. Chamá-lo tem sucesso e dispara warning event no frontend.

*Erros*
- **R7** — Toda falha tem uma **classe** de lista fechada: *transport* (sem conexão, fechada, timeout), *authentication* (ausente, inválida, expirada), *authorisation* (role recusado), *validation* (payload não cabe no layout), *not found*, *version* (R2), *rate or size limit*, *application* (o programa reportou falha), *server*. Tem **código numérico estável**, mensagem em inglês para log e campo para detalhe do programa. Classe desconhecida é lida como *server*.
- **R8** — Classe **application**: programa de backend reporta falha de negócio (pedido rejeitado, chave duplicada) com código e dados que o frontend lê no `ON EXCEPTION`, distinta das classes da plataforma. O mecanismo é decisão de `/plan` (Q1).

*Repetição e ordem*
- **R9** — Programa declara se é **seguro repetir** (idempotente). Após perda de conexão, o runtime repete **só** se o programa for declarado seguro. Senão reporta *transport* e se a chamada **pode ter sido recebida** (Q2).
- **R10** — Chamada não idempotente pode carregar **idempotency key** gerada pelo runtime; repetição escolhida pelo desenvolvedor é respondida com o resultado armazenado, por tempo declarado, sem rodar duas vezes.
- **R11** — Chamadas numa conexão rodam na ordem em que foram feitas, salvo programa declarado independente. Várias chamadas em voo (WASM-003 R6) não mudam a ordem.

*Tamanho, duração e streams*
- **R12** — Payload acima do tamanho configurado é recusado com *size limit*. Resultado legitimamente grande usa **chunked** (remontado pelo runtime, R13) ou **file transfer** (R15).
- **R13** — (Opcional) Chamada **long-running**: o backend envia mensagens de **progress** e resposta final. O frontend dispara progress event; o usuário ou o programa pode **cancelar**, e o backend honra no próximo ponto seguro.
- **R14** — (Opcional) Programa **stream** (feed, contador, log): o frontend recebe eventos, faz subscribe e unsubscribe explícitos e é reinscrito após reconexão. Cliente sem subscrição não custa nada ao servidor (Q3).

*Arquivos*
- **R15** — Upload e download fazem parte do contrato: transferência **em pedaços** com tamanho declarado, allow-list de tipos e checksum, retomável após queda, sujeita a limites de tamanho e taxa, com progress event. O backend recusa caminho ou nome escolhido pelo browser (WASM-005 R15) e nomeia os arquivos. O endereço de download é emitido por requisição, de curta duração e atrelado ao chamador (Q4).

*Evolução do contrato*
- **R16** — As regras do próprio contrato carregam uma versão: mudar handshake, lista de erros ou protocolo de arquivos é mudança de versão de protocolo, com a janela de R3.
- **R17** — Projeto Desktop não é afetado.

**Metas:** contract version em todo build e handshake na conexão; regra de compatibilidade imposta por ferramenta; modelo de erros com classes; comportamento definido para retry, chamadas grandes e longas, streams e arquivos.
**Fora do escopo:** segundo protocolo (REST, GraphQL, gRPC); hardening de transporte (WASM-003, WASM-013); mapeamento JSON em si (WASM-005); servidores multi-versão além do permitido pela compatibilidade.
**Base existente:** dois precedentes de formato de fio versionado (bincode `FormIpcMessage`, JSON lines `RemoteDebugCmd`) e a versão de manifesto `CURRENT_STRUCTURE`.

**Questões:**
- **Q1** — Como um programa reporta falha *application* — recomendado: built-in `COBOL::"…"` que o programa chama, para que a plataforma distinga falha deliberada de falha real.
- **Q2** — "Pode ter sido recebida" — aberto: declarar a garantia (at-most-once por padrão, at-least-once para programas seguros) e decidir se a classe *application* pode consultar o backend depois.
- **Q3** — Streams — aberto: exigem estado de servidor por assinante, em tensão com WASM-003 Q2. Entra no 2.0 ou fica para spec posterior.
- **Q4** — De onde baixar arquivo — recomendado: endereço HTTPS de curta duração para download; pedaços pelo WebSocket para upload.
- **Q5** — Retenção de resultados de idempotência e de interface descriptions anteriores — aberto, junto com o spec de operações.

**Estado:** rascunho. Não iniciado segundo PROGRESS (o contrato de WebSocket depende do backend, PROG-79).

---

### WASM-015 — Testes, ferramentas de agente e System KB para os tipos web

Status: rascunho. Depende de WASM-001, 004, 008 e 010. Reúne os requisitos de teste que os demais specs citam, para o harness ser construído uma vez.

**Requisitos**

*O harness*
- **R1** — **Module runner**: instancia um módulo gerado num engine WebAssembly headless com **stub host** (a host API de WASM-010 R12 implementada pelo teste). Alimenta eventos e lê escritas de propriedade, saída, erros e memória. Determinístico e rápido o bastante para o subconjunto NIST (WASM-010 R16).
- **R2** — **Browser runner**: constrói e serve um bundle e o conduz num engine de navegador headless real (click, type, wait, ler DOM e computed styles, imagem). Usado nas checagens nomeadas por WASM-002 AC1–AC2, WASM-004 AC2–AC3 e AC6–AC7, WASM-006, WASM-007, WASM-009, WASM-011 e os testes de política de WASM-013. Não pode exigir que o desenvolvedor instale navegador para os testes comuns.
- **R3** — **Backend runner**: sobe o binário do backend numa porta loopback com emissor de identidade de teste local (WASM-009 R12), conecta um cliente e exercita chamadas (WASM-003, 005, 014), incluindo os cenários de version-skew de WASM-014 AC10.

*Famílias de teste*
- **R4** — Todo controle classe A e B (WASM-004) tem teste web de round-trip de propriedade, como `tests/controls/` no desktop. Os **27 controles** que as famílias desktop ainda não cobrem são listados, com uma decisão cada: cobrir agora ou registrar como lacuna.
- **R5** — **Parity guard** estendido à web: um form de referência com um controle de cada classe é renderizado pelo engine desktop e por um navegador; comparam-se geometria e, onde o plano fixar tolerância, cor. Diferenças permitidas são só as documentadas por WASM-004.
- **R6** — **Conformance ledger** estendido, no estilo NIST (dois números por módulo: compila; roda com 0 falhas), com segunda coluna para o módulo gerado. NC, IF e IC são as primeiras linhas (WASM-010 R16). Uma regressão é um comando.
- **R7** — **Golden files** do gerador cobrem a saída web: COBOL gerado para forms web, interface description, stylesheet, host page e CSP gerada. Regeneração estilo `UPDATE_GOLDEN`.
- **R8** — Baseline do `cobolt-bench` ganha os números de WASM-010 R15 e do spec de performance: tamanho do módulo, startup, e carga fixa de aritmética e strings no módulo e no interpretador.
- **R8b** — Testes de **ciclo de vida, memória e fronteira** de WASM-017 R20, rodados para **toda combinação** de variante do módulo (multi-memory, single), variante do player (threaded, plain) e modo de execução (Worker, main thread). Cobrem: carregamento (trace de rede); descarte (abrir e fechar forms muitas vezes, voltando ao baseline); memória fixa; isolation; batching; strings; views após crescimento de memória; baseline e matriz de navegadores; yield (loop longo mantém os frames). Inclui o relatório de tamanho de WASM-016 R13 e WASM-017 R16, para que regressão de footprint seja teste que falha.
- **R9** — Cada teste reporta **resultados quantificados e legíveis** (o que foi exercitado e como se saiu), não só contagem de passes, como exige a regra de testes do projeto.

*Ferramentas para agentes*
- **R10** — Ferramentas MCP entendem os tipos de projeto: `create_project` aceita um kind (WASM-001 R10); `check` reporta o subconjunto portável e os controles classe D (WASM-002 R3, WASM-004 R5) no formato form-control-event-line; `regenerate` e `validate` cobrem os novos artefatos; `build` constrói bundle Web, pacote Mobile e binário Backend e reporta tamanhos e o resultado do secret scan; `list_files` mostra as novas entradas.
- **R11** — `render_form` e `run_form` funcionam para form web com o mesmo formato de resposta de form desktop (WASM-008 R9), e declaram qual engine produziu a imagem.
- **R12** — O servidor do IDE e o `rcrun mcp` oferecem o mesmo comportamento web. Ferramenta que não pode fazer algo num kind (construir iOS fora de macOS) diz isso e por quê.
- **R13** — O `APPLICATION_RULES` do plugin ganha as regras dos kinds web: o que um form web não pode conter; que segredos vivem só no backend; que chamadas remotas usam `CALL … WITH WEBSOCKET LINKAGE`; que form web é responsivo e usa o visual Spatial. Os skills ganham um para aplicação Web ou Mobile e um para Backend. O agente revisor passa a checar essas regras.

*Conhecimento*
- **R14** — A System KB declara, nas tabelas de documentação do compilador: classe de cada controle na web (WASM-004 R12), membros exclusivos da web, built-ins utilizáveis no navegador (WASM-010 R7), a frase `CALL … WITH WEBSOCKET LINKAGE` e seu mapeamento, os estilos de aplicação (WASM-011) e os controles Authentication e WebLink. `chunked.data` regenerado, com teste de freshness passando.
- **R15** — Especialistas do Grace não propõem construções que o kind recusa: planner e pedantic reviewer leem o kind do projeto, e as regras de política (não heurísticas de palavras) dizem o que cada kind pode usar.
- **R16** — Os **exemplos** distribuídos incluem ao menos uma aplicação Web e um Backend, construídos e rodados pela suíte. Não contêm chave nem senha (WASM-013 R7).

**Metas:** harness headless que roda módulo gerado e bundle construído, reproduzível, na suíte e no CI; contrapartes web das famílias de teste atuais; ferramentas MCP e plugin que entendem projetos web; System KB, Grace e regras do plugin que sabem o que existe na web; exemplos que também são testes.
**Fora do escopo:** automatizar a janela do próprio IDE (nunca); testar internals de identity providers e navegadores; load e soak de backends implantados (proposto em WASM-019).
**Famílias de teste atuais (§1):** `tests/controls/` (35 arquivos contra 62 tipos), o parity guard `engine_reference_form_parity_static_vs_faces` (só geometria), os golden files do codegen (`crates/cobolt-codegen/tests/golden`), o harness NIST CCVS85 com `NIST/progress.json`, o baseline de `cobolt-bench`.

**Questões:**
- **Q1** — Engine de navegador para os runners — aberto: Chromium ou Firefox headless por WebDriver ou DevTools, ou harness sobre a web view de WASM-008. O harness não pode tornar navegador pré-requisito do `cargo test`.
- **Q2** — Engine WebAssembly do module runner — aberto: interpretador puro Rust (pequeno, lento) ou JIT (rápido, maior). Remete a WASM-010 Q2.
- **Q3** — Onde ficam os dados de golden e ledger web — aberto: estender `NIST/progress.json` ou ledger irmão.
- **Q4** — Tolerância visual (R5) — aberto: só geometria (barato) ou pixels (estrito, instável entre engines). Decidir após medições.
- **Q5** — Especialista Grace web-aware (`WebFormsDesigner`) — recomendado: manter três especialistas (`FormsDesigner`, `CodeGenerator`, `EventBinder`) que leem o kind. Alternativa: criar o novo.

**Estado:** rascunho. Parte da System KB e das regras do plugin já está feita para project kinds e Preview, segundo WASM-001 e WASM-008 (ver seção 7).

---
### WASM-016 — O player: motor de forms pré-compilado para o navegador

Status: rascunho. Origem: decisão do operador (2026-10-19) sobre WASM-002 Q7 e WASM-010 Q10: dois `.wasm`. Depende de WASM-002 e WASM-010. Carrega o lado de implementação de WASM-004, 005, 006, 007, 009, 011, 013 e 018.

**Requisitos**

*O artefato*
- **R1** — O player é **um módulo WebAssembly pré-compilado**, com loader, feito em Rust + Leptos quando o produto é construído, enviado com a IDE e **copiado** (nunca compilado) em todo bundle Web e Mobile. Build Web não exige toolchain Rust, target `wasm32` nem bundler na máquina do desenvolvedor.
- **R2** — Bundle = player (variantes plain e threaded, R13c) + módulo de programa (um por projeto, multi-memory e single-memory) + host page, form descriptions, stylesheet e assets. Nada mais executa.

*A descrição de form*
- **R3** — Build escreve, para cada form, uma **form description** versionada e serializada (controles, propriedades, layout e breakpoints, eventos ligados a handlers, data bindings, referência ao stylesheet), derivada do arquivo do form (`.wcfrm` ou `.mcfrm`). O player desenha qualquer form a partir dela. A mesma descrição alimenta a Preview (WASM-008) e o Build.
- **R4** — A descrição não contém secret nem código de programa; o código dos handlers fica no módulo, referido por id.

*O que o player faz*
- **R5** — Cada controle das classes A e B (WASM-004) é implementado **uma vez**, com propriedades, eventos e métodos do modelo, a tabela propriedade → CSS (WASM-007 R1), `CssClass`, `Style`, stylesheet da form e acessibilidade (WASM-004 R10). Também implementa os controles **Authentication** (WASM-009) e **WebLink** (WASM-006).
- **R6** — O player oferece no navegador o que o form host desktop oferece: propriedades de janela e backdrop, efeitos de entrada e saída, eventos de ciclo de vida, despacho de eventos e pacing de timers e animações. Como definem as specs 037, 038 e 042, ou com diferença documentada.
- **R7** — Ação do usuário num controle: o player entrega o evento (id, controle, parâmetros) ao módulo (WASM-010 R10) e, enquanto o módulo roda, executa o que ele pede pela host API: ler e escrever propriedade, chamar método, escrever `DISPLAY`, dar hora e aleatório, fazer chamada remota.
- **R8** — Quando o módulo para numa espera (WASM-010 R9), o player faz a espera (chamada remota, diálogo, timer) e retoma com a resposta. Eventos nesse intervalo ficam em fila, em ordem (WASM-005 R9).
- **R9** — O player é **dono da conexão** com o backend: abre o WebSocket, põe o bearer token (WASM-009 R5), faz framing e correlação (WASM-003 R6, WASM-014), reconecta com back-off, renova o token e aplica timeouts. O módulo só vê "envie este JSON, devolva a resposta" (WASM-005 R8). Só conecta aos endereços configurados.
- **R10** — O player implementa o roteamento e os dois estilos (WASM-011): shell, rotas e histórico da SPA; páginas, janelas e mensagens `super` dos forms standalone.

*Consistência e versões*
- **R11** — Player, formato da form description e host API têm **cada um** uma versão. O Build copia o player que casa com o compilador. Módulo se recusa a iniciar contra player sem a entrada de host API de que precisa. Player e módulo discordantes são erro de build.

*Construir e entregar*
- **R12** — O player é construído por passo do build do produto (um workspace example, como `build_chunked_kb`), com saída commitada ou gerada no release (Q2) e freshness test. A IDE acha o player como acha seus outros assets. Perfil de build size-first, ferramentas fixadas e a regra contra `wasm-pack` são WASM-017 R16–R17.
- **R13** — Build informa o tamanho do player e das partes sob demanda (Q3). Charts, maps e Viewer carregam no primeiro uso, para que um form sem eles não pague. Os controles Canvas2D, Canvas3D e objetos gráficos (WASM-018) são partes desse tipo.
- **R13c** — Duas variantes a partir de um só fonte: **plain** (Rust estável, sem memória compartilhada) e **threaded** (nightly fixado, std reconstruída, atomics, memória compartilhada, `wasm-bindgen-rayon`; WASM-017 R16). Rayon no trabalho paralelo (imagem, charts, ordenação e filtro de DataGrid, física, rasterização de canvas), com o mesmo resultado na variante plain. A host page escolhe por `crossOriginIsolated`.
- **R13b** — Todo objeto Rust exposto ao JavaScript por `wasm-bindgen` tem descarte explícito, chamado por `unmount()` e no teardown. Os objetos expostos são poucos e listados. Panic encerra a instância com erro legível e ação de recarregar. O player nunca entrega ao JavaScript view sobre sua memória que sobreviva a uma chamada.

*Garantia*
- **R14** — O player é mantido igual ao form host desktop por medição: o form de referência de WASM-004 R11 e as verificações de ciclo de vida, efeitos e ordem de eventos rodam nos dois. Diferença é documentada ou corrigida (a deriva da spec 042 é a falha que isso previne).
- **R15** — O player obedece WASM-013: nenhuma API que insira HTML (o Viewer constrói nós DOM, WASM-013 R5), provado por busca no código; nenhum `eval`; CSP estrita; nenhuma rede além do backend e do próprio bundle; nenhum secret no player, descrições ou stylesheet.
- **R16** — Com a Preview (WASM-008) aberta, o player aceita novo módulo, form description ou stylesheet sem recarregar, e a Preview atualiza no lugar.
- **R17** — Projeto Desktop não muda; o form host desktop continua sendo o host das aplicações desktop.

**Metas:** player pré-compilado único que desenha qualquer form, sem Rust por projeto; implementação única de todo controle web e dos comportamentos do form host; form description, host API e tripla de versões definidas; o player é dono da conexão, então o módulo não tem rede; enviado com a IDE.
**Fora do escopo:** código Leptos por projeto e componentes escritos pelo desenvolvedor; `Custom { plugin_id }` na web na v1; SSR; substituir o canvas egui do Form Designer; biblioteca pública de componentes.
**Estado do player (§1):** o form host desktop (`cobolt-form-host`) não é reutilizável no navegador (depende de `rfd`, `pollster` e eframe). Não existem Leptos, `wasm-bindgen`, `web-sys` nem `trunk` no workspace. Precedente de asset commitado com freshness test: `assets/knowledge/chunked.data`.
**Textos fixos do player** (carregando, erro, tentar de novo) são da host page (WASM-002 R15) e traduzíveis pelo desenvolvedor.
**Critérios que acrescentam detalhe:** AC5 (com o backend parado, o player reconecta com back-off e o programa vê `ON EXCEPTION` só para a chamada em voo); AC8 (form sem chart, map ou Viewer não baixa as partes deles).

**Questões:**
- **Q1** — Formato da form description — recomendado: JSON versionado, com binário compacto depois se o tamanho importar.
- **Q2** — Player commitado ou construído no release — recomendado: commitado, com freshness test (como `chunked.data`).
- **Q3** — Um arquivo ou partes sob demanda — recomendado: partes desde o início, pelos controles classe B.
- **Q4** — Quanto do form host chega ao navegador na v1 (efeitos, estados e posições de janela) — aberto: decidir por comportamento no `/plan`; manter, mapear ou descartar.
- **Q5** — Extensibilidade (`Custom`, external crates na web) — aberto, fora de escopo.
- **Q6** — Player em duas builds da IDE — aberto: confirmar que rebuild usa sempre o player atual e se um projeto pode fixar um player antigo.

**Estado:** rascunho. **Decisão pendente do operador:** o uso de Leptos no player (PROG-44). Ver seção 7.

---

### WASM-017 — Arquitetura do módulo, modelo de memória, threads e fronteira JavaScript

Status: rascunho, com as decisões do operador de 2026-10-20 incorporadas. Depende de WASM-002, 010 e 016. Ajusta WASM-002, 010, 011, 013, 015 e 016.

**Requisitos**

*Carga e vida do form*
- **R1** — Build emite **um módulo de programa para o projeto inteiro**, com o Common Code de cada form copiado, autocontido (WASM-010 R2). O loader o busca com compilação em streaming e inicia o form principal assim que está pronto.
- **R2** — Cada instância de form tem seu `WORKING-STORAGE` numa região da memória de dados, alocada na criação e liberada na destruição (spec 049: não residente e não preservado). Tamanho fixo em compilação (R7): dados estáticos mais, para cada form, `Instances` × seu tamanho. `Instances` é propriedade do form (padrão 4 para form que `OpenFormSync`/`OpenFormAsync` possam abrir; 1 nos demais). Criar instância além de `Instances` levanta o erro de execução de WASM-010 R11 com o nome do form.
- **R3** — Loader por `fetch` + instanciação em streaming. Integração ES-module é **opcional**, usada só por feature-detection (R6, R18). Partes pesadas do player (WASM-016 R13) são buscadas no primeiro uso pelo mesmo loader.
- **R4** — O tempo de vida segue o COBOL, não a pilha: o `WORKING-STORAGE` de um programa vive do primeiro `CALL` até `CANCEL` ou até a instância do form ser destruída. Exceção: programa `INITIAL`, que começa limpo a cada chamada e tem o estado descartado ao retornar.
- **R5** — Ao destruir uma instância, o módulo libera e reinicializa sua região, e o loader solta toda referência (listeners, timers, chamadas pendentes, entradas da tabela do host). Ao desmontar a aplicação, o loader solta o módulo e as memórias e termina os Workers. Build de debug prova os dois (R20).
- **R6** — **Component Model não é usado na v1** (suporte do Chrome não estabelecido). A host API continua em imports e exports simples. Revisitado quando os três motores tiverem suporte nativo (Q7).

*Modelo de memória*
- **R7** — Memória de dados de tamanho fixo em compilação (R2). Código gerado sem `memory.grow`. Dado que não pode ser dimensionado é erro de compilação que nomeia o item.
- **R8** — Frames de texto e JSON na **transport arena** de tamanho fixo, tirado do limite de payload do projeto (WASM-005 R11). Multi-memory: arena é a memória 1, separada dos dados (memória 0). Single-memory: região reservada da memória 0. A host API é a mesma: offset, length e seletor de memória.
- **R9** — **Multi-memory é o padrão** do módulo (operador, 2026-10-20). O Build produz também a variante **single-memory** (sem nenhuma instrução multi-memory), usada sempre para iOS (multi-memory desligado por padrão) e sempre que a matriz de browsers inclua Safari ou iOS web view (padrão). O bundle Web leva as duas e o loader sonda `WebAssembly.validate` e carrega a adequada. Projeto sem Safari pode escolher "multi-memory only"; o Build recusa isso se a matriz inclui Safari (AC7). Só o módulo emitido usa uma segunda memória; o player (Rust) não.
- **R10** — O host **nunca guarda** `TypedArray` ou `DataView` sobre a memória do módulo entre chamadas: pega a view a cada uso. O player não entrega ao JavaScript views que sobrevivam a uma chamada; vale também para memória compartilhada.

*A fronteira*
- **R11** — Host API de **granularidade grossa**: escritas de propriedade de um handler são acumuladas e entregues numa chamada ao fim ou à suspensão, com leituras refletindo as escritas anteriores; um evento é uma chamada; uma chamada remota é uma requisição e uma resposta; **nenhuma chamada ao host por iteração de laço**. Programa de desenho segue a mesma regra: comandos vão a um buffer na arena e são executados numa chamada (WASM-018 R6).
- **R12** — Texto cruza a fronteira **uma vez em cada direção**: UTF-8 na arena contra strings JavaScript por `TextEncoder`/`TextDecoder`. Onde o navegador oferece JS String Builtins, o loader pode usá-los (feature-detected).
- **R13** — Dados que não precisam do programa (bytes de arquivo solto, imagem, buffers de pixels dos objetos gráficos) não passam pelo módulo: o player os move (WASM-014 R15, WASM-018).
- **R14** — Todo objeto Rust exposto ao JavaScript por `wasm-bindgen` tem descarte explícito (`free()` ou dispose), chamado por `unmount()` e pelo loader no teardown. Objetos poucos e listados; nenhum fica só ao GC do JavaScript. Vale para os handles dos objetos gráficos (WASM-018).
- **R15** — Panic encerra aquela instância de forma limpa: a host page mostra erro legível com ação de recarregar e levanta evento de erro. Erro de execução COBOL não é panic (WASM-010 R11). Na variante threaded, panic num worker também encerra a instância.

*Build e baseline*
- **R16** — Player com perfil de release **size-first**: `lto`, `codegen-units = 1`, `panic = "abort"`, símbolos removidos, `opt-level` por medição (`"z"` contra `"s"`) e passo de `wasm-opt`; tamanhos reportados (WASM-016 R13). Duas variantes do mesmo fonte: plain (Rust estável, baseline) e threaded (nightly fixado, std reconstruída, atomics, bulk memory, memória compartilhada, `--target web`, `wasm-bindgen-rayon`). O resto do workspace fica em Rust estável, MSRV 1.92; o nightly fica confinado ao passo do player.
- **R17** — O build do produto roda `wasm-bindgen` CLI fixado, Binaryen `wasm-opt` fixado e o bundler (WASM-002 Q2), **nunca `wasm-pack`**. Essas ferramentas e o nightly existem só no build do produto; valem as regras de supply chain (WASM-013 R9); a data do nightly é registrada em todo build.
- **R18** — Baseline de recursos WebAssembly e matriz de browsers (Chrome e Edge, Firefox, Safari, iOS web view, Android WebView), a partir de MDN e Can I Use no `/plan`. O loader sonda e escolhe, independentemente: variante do módulo (R9), variante do player (R19) e aceleração opcional (JS String Builtins, ES-module). Abaixo do baseline mostra a mensagem de WASM-002 R15.

*Concorrência*
- **R19** — **Threads e Rayon estão na v1** (operador, 2026-10-20):
  - (a) Rayon roda no player e nos objetos gráficos (filtros e reamostragem de imagem, rasterização de chart e canvas, física, ordenar e filtrar DataGrid), num pool dimensionado por `navigator.hardwareConcurrency` (limitado, configurável), no player threaded;
  - (b) o módulo roda num **Worker dedicado** quando a página é cross-origin isolated; chamadas ao host são proxied à thread principal (escritas em lote, leituras por espera em memória compartilhada), para que um loop longo não congele a página. Sem isolamento, o módulo roda na thread principal com yield cooperativo (WASM-010 R9). Os resultados não dependem do modo;
  - (c) o isolamento (`COOP: same-origin` com `COEP: require-corp` ou `credentialless`) é exigido para threads e é propriedade de deployment: o Build imprime os headers e oferece um service-worker shim opcional (desligado por padrão); recursos de outras origens precisam permitir (CORS ou CORP);
  - (d) página que embute a aplicação só é isolada se a host page for; senão cai para o player plain e a thread principal;
  - (e) nada exige threads para correção: todo recurso funciona no player plain e na thread principal, e um teste roda os mesmos casos em cada (R20);
  - (f) resultados determinísticos: redução Rayon não pode depender do número de threads (WASM-018 R20).
- **R20** — Suíte de testes para cada combinação de variante do módulo, variante do player e modo (Worker, thread principal): **loading** (módulo buscado no início; parte pesada só no primeiro uso); **disposal** (abrir e fechar form muitas vezes volta à linha de base; unmount termina os Workers); **fixed-memory**; **batching** (handler com 1000 propriedades faz no máximo um número declarado de chamadas); **strings** (ida e volta UTF-8 com multi-byte); **view** (chamada que faz crescer a memória do player deixa as leituras corretas); **baseline e matriz** (browser abaixo do baseline recebe a mensagem); **isolation** (com e sem COOP/COEP, a variante certa é escolhida e os resultados são os mesmos); **yield** (loop longo mantém os frames dentro de um prazo na thread principal).

**Metas:** módulo que faz streaming, compila enquanto baixa e inicia rápido, com partes pesadas sob demanda; memória de form devolvida ao destruir, provada por teste; modelo de memória sem crescimento e sem views destacadas; multi-memory por padrão e single-memory onde faltar; host API barata de cruzar; concorrência real onde a plataforma permite, nunca exigida para correção; build enxuto e reprodutível, com baseline e matriz impostos pelo Build.
**Fora do escopo:** threads na semântica do COBOL; Component Model, WebAssembly GC e memórias de 64 bits na v1; números de desempenho (a spec só os torna mensuráveis).

**Tabela de disposição (§1):** 1a carga sob demanda (um módulo; partes pesadas do player: charts, maps, Viewer, motor 3D); 1b Component Model não mantido; 1c ES-module opcional e detectado; 1d "destruir ao retornar" adaptado ao COBOL (região por form, `INITIAL` descartado); 2a JS String Builtins opcionais; 2b multi-memory padrão com a arena como memória 1; 3 `.free()` para o player e objetos gráficos; 4 sem `memory.grow` no módulo; 5a chamadas grossas como regra; 5b threads e Rayon adotados; 6 perfil size-first e `wasm-opt` para o player, nightly fixado para a variante threaded.

**Questões:**
- **Q1** — Um módulo por projeto — **decidido** (2026-10-20). Custo: primeiro download maior, mitigado por streaming, compressão e partes sob demanda.
- **Q2** — Orçamento de yield (ms) e número padrão de Workers — aberto: sugestão `hardwareConcurrency` menos um, limitado a 8.
- **Q3** — Variantes single/multi-memory — **decidido** (R9).
- **Q4** — Matriz de browsers e versões mínimas (em especial o iOS mais antigo) — aberto, a partir do `/plan`.
- **Q5** — Isolamento no shell mobile (headers do asset protocol do Tauri; se o iOS web view isola) — aberto. Se não isolar, roda o player plain e a thread principal.
- **Q6** — `wasm-opt` — aberto, recomendado: só no build do player.
- **Q7** — Revisitar Component Model e ES-module — aberto; gatilho: suporte nativo em Chrome, Firefox e Safari.
- **Q8** — `opt-level` — aberto: `"z"` ou `"s"` por medição.
- **Q9** — Nightly: data fixada e se `build-std` ainda é exigido — aberto.
- **Q10** — Service-worker shim de headers — aberto: oferecer, só documentar ou deixar de fora.

**Estado:** rascunho. Threads, multi-memory e player threaded entram no player (PROG-45 a PROG-50).

---

### WASM-018 — Canvas 2D, Canvas 3D e objetos gráficos, em todas as plataformas

Status: rascunho (parte 1 do plano implementada; ver seção 7). Origem: operador, 2026-10-20: controles Canvas2D e Canvas3D e objetos para gerar gráficos (jogos, charts, manipulação de imagem), em todas as plataformas, com crates populares e 100 % Rust. Depende de WASM-004, 010, 016 e 017.

**Requisitos**

*O catálogo*
- **R1** — Toda toolbox oferece dois controles visuais, **Canvas2D** e **Canvas3D**, cada um com propriedades, eventos, métodos, tamanho e ícone padrão, seções do Properties, round-trip `.cfrm`, documentação no System KB, teste em `tests/controls/`, rótulos em seis idiomas e nomes de método no vocabulário fechado.
  - **Canvas2D:** `Width`, `Height` (pixels lógicos), `PixelRatio`, `Smoothing`, `BackgroundColor`, `TargetFps` (0 = redesenho sob demanda), `AutoPresent`, `AccessibleLabel`; eventos `onFrame` (com milissegundos decorridos), `onPointerDown`/`Move`/`Up`/`Enter`/`Leave`, `onWheel`, `onKeyDown`/`Up`, `onTouchStart`/`Move`/`End`, `onResize`.
  - **Canvas3D:** as mesmas propriedades e eventos, mais `ClearColor`, `Msaa` e `Backend` somente leitura (`WebGPU`, `WebGL2`, `Vulkan`, `Metal`, `Dx12`, `Software`).
- **R2** — Cinco objetos não visuais, soltos num form como `Timer` ou `RestClient`: **ImageBuffer** (R11), **Plot** (R12), **Scene3D** (R9), **Physics2D** e **Physics3D** (R10). O que eles criam (série, nó, luz, corpo) são **handles**: inteiros que `RETURNING` traz a um data item e que os métodos recebem. Handle é liberado por `Release`, pela remoção do que nomeia, e na destruição do objeto ou form (WASM-017 R14).
- **R3** — Tudo em **um** crate Rust puro, `cobolt-graphics`, sem egui, `web-sys` nem sistema de arquivos. Compila para os targets do host e para `wasm32` nas duas variantes do player. Host desktop e player só fornecem superfície, eventos de entrada, relógio de frames e bytes de assets.

*100 % Rust*
- **R4** — Um crate só entra se: é Rust; nenhum build script do conjunto padrão de features compila C/C++ ou liga biblioteca de sistema (`cc`, `cmake`, `bindgen`, `pkg-config`), exceto as APIs gráficas do navegador e do SO que o `wgpu` alcança por bindings Rust; licença permissiva compatível com Apache-2.0; release nos últimos doze meses; suporta `wasm32`; MSRV ≤ 1.92 (exceto o nightly do player threaded); passa os dependency guards de WASM-013 R9 (sem `ring`, sem `aws-lc-rs`). Deny list de teste: `font-kit`, `freetype-sys`, `harfbuzz-sys`, `skia-safe`, `cairo-sys-rs`, `libwebp-sys`, `mozjpeg-sys` e semelhantes.
- **R5** — Crates escolhidos (salvo se o plano mostrar que um falha R4):
  - Rasterizador 2D: `tiny-skia` (CPU, paths, strokes, dashes, gradientes, patterns, clip masks, blend modes; determinístico);
  - Curvas: `kurbo`;
  - Texto: `skrifa` + `rustybuzz` + `fontdb` (contornos, shaping de latim, CJK, árabe e índico, busca de fontes), com `unicode-bidi` e `unicode-linebreak`;
  - Imagens: `image` + `imageproc` + `fast_image_resize` (codecs Rust puro, filtros, morfologia, reamostragem);
  - Charts: `plotters` com feature de texto `ab_glyph` e `DrawingBackend` próprio sobre `tiny-skia`;
  - 3D: `wgpu` + `naga` + `glam` + `bytemuck` + `gltf` (WebGPU e WebGL2 no navegador; Vulkan, Metal e Dx12 nos demais); a camada de cena é nossa;
  - Física: `rapier2d` e `rapier3d` (com `parry`), com `parallel` via Rayon e `enhanced-determinism`;
  - Paralelismo: `rayon` (no navegador via `wasm-bindgen-rayon`);
  - Gamepad: `gilrs` no desktop; a Gamepad API do navegador pelo player.
  - Rejeitados, com razão: `bevy` (framework de aplicação, dezenas de MB de Wasm, quer o loop); `three-d` (segunda pilha gráfica ao lado do wgpu); `macroquad`/`miniquad` (donos da janela e do loop); `charming` (renderiza com JavaScript); `skia-safe` e `cairo-rs` (C ou C++); `vello` (GPU ainda não estável, substituível depois atrás da mesma API).

*Display list e paridade*
- **R6** — Os métodos de desenho registram comandos numa **display list** binária versionada, definida uma vez em `cobolt-graphics`, executada pelo controle em uma chamada: em `Present`/`Flush`, no fim de `onFrame` com `AutoPresent`, e antes de qualquer método que leia a imagem (`ToImage`, `GetPixel`, `MeasureText`, `IsPointInPath`). No módulo gerado, a lista fica na transport arena e uma chamada ao host a entrega (WASM-017 R11); no interpretador, fica em memória do host. Todo host decodifica com o mesmo Rust.

*Canvas2D*
- **R7** — Canvas2D com a semântica da HTML Canvas 2D API onde há equivalente (tabela de diferenças na KB):
  - **Estado:** `Save`, `Restore`, `Clear`.
  - **Paths:** `BeginPath`, `MoveTo`, `LineTo`, `QuadTo`, `CubicTo`, `ArcTo`, `Arc`, `Ellipse`, `Rect`, `RoundRect`, `ClosePath`, `Fill`, `Stroke`, `Clip`.
  - **Formas:** `FillRect`, `StrokeRect`, `ClearRect`, `DrawLine`, `DrawCircle`, `DrawPolygon`.
  - **Estilo:** `SetFillColor`, `SetStrokeColor`, `SetLineWidth`, `SetLineCap`, `SetLineJoin`, `SetDash`, `SetAlpha`, `SetBlendMode`, `SetLinearGradient`, `SetRadialGradient`, `SetPattern`, `SetShadow`.
  - **Transformações:** `Translate`, `Rotate`, `Scale`, `SetTransform`, `ResetTransform`.
  - **Texto:** `SetFont`, `SetTextAlign`, `DrawText`, `MeasureText`.
  - **Imagens:** `DrawImage` de um ImageBuffer (inteiro ou retângulo de origem, com ou sem escala); `DrawSprite` para frame de sprite sheet.
  - **Leitura de volta:** `ToImage`, `GetPixel`, `IsPointInPath`.
  - **Apresentação:** `Present`.
  - Rasterização em CPU, com anti-aliasing e determinística (`tiny-skia`); canvas grande pode ser rasterizado em tiles em paralelo (R20).
- **R8** — Texto **modelado** (shaped), bidirecional e quebrado corretamente nos seis idiomas, incluindo japonês e chinês, com fontes do conjunto base embutido ou de `Assets/`. `MeasureText` retorna exatamente o que `DrawText` ocupa.

*Canvas3D e Scene3D*
- **R9** — Canvas3D renderiza uma Scene3D com `wgpu`: `SetScene`, `SetCamera`, `Render`, `Resize`, `Screenshot` (para um ImageBuffer) e `Pick` (nó sob um pixel, por ray casting via `parry3d`). Scene3D oferece:
  - Primitivas: `AddBox`, `AddSphere`, `AddPlane`, `AddCylinder`, `AddCone`, `AddTorus`;
  - `LoadModel` de glTF 2.0 ou GLB, com malhas, materiais, transformações e animações esqueléticas e de nós;
  - Transformações: `SetPosition`, `SetRotation`, `SetScale`, `LookAt`;
  - Materiais: cor, metallic, roughness, emissive, ImageBuffer como textura, PBR metallic-roughness;
  - Luzes: `AddDirectionalLight`, `AddPointLight`, `AddSpotLight`, `SetAmbient`, com shadow maps para direcional e spot;
  - `SetBackground`, `SetFog`; câmeras `AddCamera` (perspectiva e ortográfica); `SetAnimation`/`PlayAnimation`; `Remove`; instancing; frustum culling.
  - `Backend` nomeia o que roda (WebGPU, depois WebGL2 na web; Vulkan, Metal ou Dx12 nativo; software em testes). Sem backend, mostra mensagem legível e levanta evento.
- **R10** — **Physics2D** e **Physics3D**: mundo (`CreateWorld` com gravidade); corpos estáticos, dinâmicos e cinemáticos com formas (ball, box, capsule, convex hull, trimesh em 3D); `SetPosition`, `SetVelocity`, `ApplyForce`, `ApplyImpulse`; juntas (fixed, revolute, prismatic); `RayCast`; `Step` com time step fixo opcional; `GetPosition`, `GetRotation`; eventos de colisão; `Bind` de corpo a sprite ou nó de Scene3D.

*Objetos de imagem e chart*
- **R11** — **ImageBuffer**: `Create`, `LoadAsset`, `LoadFromBytes`, `Encode` (PNG, JPEG, GIF, WebP, BMP, QOI), `Save` (desktop grava arquivo; web entrega ao navegador ou ao backend), `Resize` (nearest, bilinear, Catmull-Rom, Lanczos), `Crop`, `Rotate`, `Flip`, `Fill`, `GetPixel`, `SetPixel`, `DrawOnto` um Canvas2D, `Composite` com blend modes, e filtros: grayscale, invert, brightness, contrast, hue, saturation, threshold, blur (Gaussian, box), sharpen, edge detection, `Convolve` com kernel, dilate, erode, histogram. Decodifica só com codecs Rust puro, com limite de pixels.
- **R12** — **Plot** com `plotters`: line, bar, scatter, area, pie, histogram, candlestick, heatmap, polar, error bars, superfície e dispersão 3D. Dados de `AddPoint` ou de tabela COBOL (`SetSeriesFromTable`: contagem, stride e tipo numérico descritos uma vez, valores lidos numa passada, WASM-017 R11). Títulos, eixos, legendas, grades e estilos. `Render` num Canvas2D, num ImageBuffer ou como texto SVG.

*Entrada, frames e acessibilidade*
- **R13** — Controles canvas são focáveis e entregam teclado, ponteiro, roda, toque e gamepad como eventos **e** como estado consultável (`IsKeyDown`, `PointerX`, `PointerY`, `GamepadAxis`, `GamepadButton`); expõem `AccessibleLabel` e descrição para tecnologia assistiva; o programa pode definir alternativa de texto.
- **R14** — Com `TargetFps` acima de zero, o canvas levanta `onFrame` nessa taxa com o tempo decorrido, ritmado pela tela (`requestAnimationFrame` no navegador, repaint do egui no desktop). Pausa quando a página ou janela está oculta, limita em bateria baixa. Modo fixed-step dá a um jogo `onFrame` estável para a física.

*Segurança e limites*
- **R15** — Limites (tamanho do canvas, pixels decodificados, tamanho de modelo e textura, nós e corpos, comprimento da display list), definidos pelo projeto com padrões seguros; além deles, erro legível. Não leem arquivo nem abrem conexão (assets embutidos; remoto passa pelo backend). Decodificação de imagem e modelo não confiáveis com codecs Rust puro, **fuzzed** (WASM-013 R15). Shader ou modelo não derruba a página; dispositivo gráfico perdido é tratado e reportado.

*Plataformas*
- **R16** — Todo host que roda form implementa controles e objetos: o form host egui (Preview da IDE, Run Form, aplicações compiladas), em que Canvas2D envia pixels como textura e Canvas3D renderiza numa textura; e o player (Web e Mobile). O interpretador e o módulo gerado implementam os métodos e montam a display list (WASM-010 R7). Paridade interpretador-binário vale para todos.
- **R17** — O Form Designer mostra canvas como placeholder rotulado com tamanho e backend. Preview, Run Form e aplicação construída mostram o desenho ao vivo.
- **R18** — O player carrega o motor 3D, o código de charts e os filtros de imagem como partes no primeiro uso (WASM-016 R13, WASM-017 R3). O Build informa os tamanhos.
- **R19** — Projeto de exemplo `examples/PowerGraphics` acompanha o produto com jogo (sprites, física, entrada), galeria de charts, image lab e visualizador 3D, construído e executado em Desktop e Web na suíte. O capítulo do Developer's Guide é escrito no release.

*Determinismo e paralelismo*
- **R20** — Trabalho com Rayon (filtros e reamostragem, render de plot, tiling de canvas, broad phase de física) dá **o mesmo resultado para qualquer número de threads**, inclusive uma: trabalho por pixel é independente, reduções em ordem fixa, física com `enhanced-determinism`. Testes comparam uma thread contra várias e o plain contra o threaded; golden images são exatas para o rasterizador CPU e dentro de tolerância declarada para a GPU.

**Metas:** Canvas2D e Canvas3D mais ImageBuffer, Plot, Scene3D, Physics2D e Physics3D em Desktop, Web e Mobile, com a mesma API e os mesmos resultados; implementações 100 % Rust de crates populares; display list que mantém a fronteira grossa; todos os núcleos quando a plataforma oferece, com resultado idêntico quando não; projeto de exemplo que exercita tudo.
**Fora do escopo:** áudio (o desktop já toca som pelo Animator); game engine geral (ECS, scheduler, pipeline de assets); renderizador 2D por GPU (caminho GPU seria mudança interna); substituir os sete controles de chart, PictureBox ou Animator; ray tracing, compute shaders para o desenvolvedor, VR e AR.
**Estado atual (§1):** 62 tipos em `ControlType::ALL`; `methods.rs` exige o nome no vocabulário fechado, senão vira subscript; `CALL … RETURNING` devolve valor desde 2026-10-19 (é como um handle volta ao COBOL). Crates já na árvore: `tiny-skia`, `kurbo`, `ab_glyph`, `skrifa`, `image`, `wgpu`, `naga`, `glow`. Ausentes: `imageproc`, `glam`, `lyon`, `plotters`, `rapier`, `gltf`, `rustybuzz`, `cosmic-text`.

**Questões:**
- **Q1** — Rasterizador — **decidido** (revisitável): `tiny-skia` em CPU. Alternativa: `vello` (GPU) quando estável.
- **Q2** — Camada de cena — **decidido**: própria sobre `wgpu`.
- **Q3** — Animação esquelética e sombras — **decidido** (estão em R9).
- **Q4** — Renderizador desktop — aberto: `eframe` tem glow e wgpu; compartilhar o dispositivo com o Canvas3D exige wgpu. (O texto do spec chama isso de "Q7" em R16 e de "Q4" no §7; registro as duas referências como estão.)
- **Q5** — Adaptador de software para CI — aberto: para os goldens 3D rodarem sem GPU.
- **Q6** — Nomes — **decidido**: `ImageBuffer` e `Plot` (evitando os controles `Image` e `Chart`).
- **Q7** — Fontes — aberto: quais entram no conjunto base para CJK e RTL, e se carregam sob demanda.
- **Q8** — Áudio — aberto, fora desta spec: objeto de som sobre `kira` seria spec separado.

**Plano (plan.md e tasks.md):** tasks **T1–T22**.
- **Parte 1 (o crate), feita:** T1 fundação (display list v1, rasterizador, limits, handles); T2 motor de texto; T3 ImageBuffer; T4 Plot; T5 Physics2D/3D; T6 Scene3D e Canvas3D; T7 API voltada ao COBOL, registry e handles com dono; T8 dependency guard; T9 medições e sondas de tamanho; T10 plano e tarefas.
- **Parte 2 (hosts), não iniciada:** T11 adaptador do host egui; T12 bindings do interpretador (objetos como controles, vocabulário em `methods.rs`, `RETURNING` para handles); T13 gerador e módulo gerado (display list na arena, uma chamada ao host); T14 partes do player (carregar motor 3D, charts e filtros no primeiro uso).
- **Parte 3 (superfície do produto), não iniciada:** T15 controles Canvas2D e Canvas3D (propriedades, eventos, `.cfrm`, Properties, toolbox, `tests/controls/`); T16 placeholder do Designer; T17 System KB e `chunked.data`; T18 rótulos em seis idiomas; T19 gamepad (`gilrs` e Gamepad API); T20 `examples/PowerGraphics`; T21 verificação em navegador e entre plataformas; T22 docs e finalização.

**Divergências registradas no plano (versus o spec):** `harfrust` 0.12 em vez de `rustybuzz` (arquivado upstream); `icu_segmenter` em vez de `unicode-linebreak` (último release de 2023); `tiny-skia` 0.12 em vez de 0.11.4 (o gradiente cônico de dois pontos é o de `createRadialGradient`); `plotters` com backends próprios e motor de texto próprio em vez de `ab_glyph`; `glam` 0.33; `gltf` 1.4.1 sem a feature de leitura de arquivo; `naga` só como dev-dependency; threads do `rapier` limitadas a 256.

**Pendências do plano:** não verificado a regra de idade de release de R4 (datas indisponíveis offline). Goldens de GPU diferem entre adaptadores (só Metal foi rodado).

**Critérios de aceite (AC1–AC14):** AC1 (desenho igual pixel a pixel nos três destinos, texto nos seis idiomas); AC2 (dez mil formas numa só chamada ao host); AC3 (`MeasureText` igual à extensão de `DrawText` em latim, japonês, chinês e RTL); AC4 (glTF animado com luzes, sombras e pick); AC5 (`Backend` reporta WebGPU, WebGL2 ou nenhum); AC6 (mesmas posições com 1 ou N threads e no plain contra o threaded); AC7 (blur de 24 MP mais rápido no threaded, medido); AC8 (tabela COBOL de 100.000 valores vira série numa passada); AC9 (abrir e fechar form com canvas, imagem e cena 100 vezes volta à linha de base); AC11 (dependency guard, plano T8); AC10, AC12 e AC13 não estão descritos no resumo de leitura, ver o spec; AC14 (exemplo web carrega o motor 3D só quando o form 3D aparece).

---
---

## 4. Specs propostos, ainda sem arquivo (WASM-019 a WASM-035)

Fonte: WASM-INDEX §5. "P2" é necessário logo depois ou junto com a série; "P3" é depois. Os números são provisórios. Nenhum tem arquivo de spec.

| Proposto | Spec | Por que falta hoje | P |
|---|---|---|---|
| WASM-019 | **Deploy e operação do backend** | Containers, nuvem (PowerCloud já conhece AWS), reverse proxy e TLS, escala e afinidade de sessão, secrets, métricas e traces (estendendo o documento de observabilidade), health, upgrades graduais, migrações e backups | P2 |
| WASM-020 | **Acessibilidade e internacionalização das aplicações geradas** | ARIA, ordem de foco, contraste, RTL, formatos de locale contra `PICTURE` editado e `DECIMAL-POINT IS COMMA`, IME, UTF-8 em `PIC X`/`PIC N` e JSON, troca de idioma em execução | P2 |
| WASM-021 | **Modelo de interação por toque e mobile** | Gestos, long-press como menu de contexto, sem hover, lint de tamanho mínimo de alvo, comportamento do teclado virtual, modos de entrada pelo `PICTURE` (teclado numérico), orientação, pull-to-refresh | P2 |
| WASM-022 | **Offline, PWA e cache** | Service worker, fila de chamadas offline, política de IndexedDB, fluxo de atualização do bundle e cache busting, instalabilidade | P2 |
| WASM-023 | **Controles de dados além da v1** | Paginação, ordenação e filtro no servidor para DataGrid; controle otimista no Save; transações entre chamadas (`COMMIT`/`ROLLBACK` pelo canal); locking de registro com clientes remotos (depende do fix de cross-process locking) | P2 |
| WASM-024 | **Depuração e diagnóstico** | Breakpoints COBOL em execução no browser (o crate DAP sobre WebSocket até a IDE), source maps além da line table, telemetria de console e erros, crash reports, correlation ids entre logs do frontend e do backend | P2 |
| WASM-025 | **Distribuição e atualizações** | Hospedagem e CDN, headers de cache, URLs versionadas, updater do Tauri, checklists de submissão às lojas, o que o `update_check` da IDE faz quando existe 2.0.0 (ofereceria a todo IDE 1.x.x), rollback | P2 |
| WASM-026 | **Conversão de projeto e compartilhamento de código** | Assistente de conversão desktop → web que aponta controles e verbos sem suporte, passos do `project_upgrade`, Common Code e copybooks compartilhados entre desktop e web | P2 |
| WASM-027 | **Templates e exemplos** | Template *Agentic AI Chatbot (Web)* (a chave do modelo fica no backend), *UserControl (Web)*, versões web de PowerChat, PowerAnalytics, PowerDemo3 e PowerSpatial, *Create from Template…* | P2 |
| WASM-028 | **Documentação, localização e plano de release** | Capítulos do Developer's Guide 2.0 (retidos pelo ruling de 2026-10-19), README, regeneração das cinco traduções no minor, anúncios no fórum agrupados no RC, passos de release candidate a partir do 2.0.0 | P2 |
| WASM-029 | **Orçamentos de desempenho** | Números que WASM-017 torna mensuráveis: tamanho e startup do módulo, vazão do módulo contra o interpretador (`cobolt-bench`), virtualização do DataGrid, memória, tamanho de payloads | P2 |
| WASM-030 | **Licenciamento e conformidade** | Licenças de Leptos, Tauri, crate de web view, emitter e bundler; políticas de uso de tiles de mapa; termos e consentimento do reCAPTCHA; formulários de data-safety das lojas; leis de proteção de dados | P2 |
| WASM-031 | **Extras de identidade** | Login social e federação, passkeys/WebAuthn, modelo de autorização (papéis e permissões), gestão de sessão e dispositivo, log de auditoria | P3 |
| WASM-032 | **Plug-ins, controles customizados e external crates em `wasm32`** | `Custom { plugin_id }` sem painter nem runtime; `EXEC RUST` recusado no browser (WASM-002 Q4) e precisa de caminho pelo backend | P3 |
| WASM-033 | **Mais controles nativos da web** | Vídeo e áudio embutidos, imagem de URL, IFrame, editor de rich text, câmera e leitor de QR (mobile) | P3 |
| WASM-034 | **Outros form factors** | Smartwatch Android, Apple Watch, TV | P3 |
| WASM-035 | **Evolução do designer** | Designer baseado em web view, designer ciente de CSS, molduras de dispositivo, preview de estado de componente | P3 |

**Lacunas estratégicas que o próprio índice aponta** (afetam todos os specs):
1. O princípio "um renderer para tudo" (spec 017) termina na web: o Form Designer pinta com egui, a web com o DOM. Paridade de pixel é meta medida (WASM-004 R11), e o operador decide quanta diferença é aceitável (WASM-004 Q6, WASM-007 Q5).
2. **Uma linguagem, dois motores**: o browser roda WebAssembly gerado; backend e desktop rodam o interpretador. Manter os dois iguais é gate medido (WASM-010 R16, WASM-015 R6).
3. O **gerador** é o caminho crítico: a fase A de WASM-010 precisa existir antes da maioria dos critérios de aceite dos outros specs.
4. **Chaves em forms**: `AgentAPIKey` e `ConnectionString` aceitam texto. Os exemplos de 2026-10-19 deixam as chaves vazias, e o gate de secret scan (WASM-002 R7, WASM-013 R7) é o que mantém isso.

---

## 5. Contrato de baixo nível — WASM-ABI-v1 (152 itens)

Estado: draft 1, amended em 2026-10-20. É normativo para `cobolt-wasm` (produtor), `cobolt-web-player` e o loader da host page (consumidores). Mudar o arquivo é mudar os três. Campo `abi` no manifest sobe quando uma regra muda de forma incompatível. Hoje `abi: 1`.

**Convenções (ABI-03 a ABI-06):** inteiros little-endian; `varint` é LEB128 sem sinal, até 5 bytes; `text` é `varint length` seguido de bytes UTF-8; offsets são bytes dentro da **arena**.

**Artefatos (§1):**
- `program.wasm` — variante multi-memory; memória 0 = dados COBOL, memória 1 = arena (ABI-07);
- `program.single.wasm` — variante single-memory; arena é região da memória 0; sem instrução multi-memory (ABI-08);
- `player.wasm`, `player.js` — construídos pelo produto; plain e threaded (ABI-09);
- `manifest.json` — também embutido como custom section `prmanifest` (ABI-10);
- forms, stylesheet e assets (ABI-11);
- regra dos imports: só os do §3 (ABI-12); regra dos exports: só os do §4, mais as memórias e os `pr_dbg_*` opcionais (ABI-13).

**Memória (§2):**
- Memória de dados 0 com tamanho fixo, mínimo e máximo iguais (ABI-14); sem `memory.grow` (ABI-15);
- layout: `[constants][descriptors][form templates][instance regions: Instances × size per form][task stacks][scratch registers]` (ABI-16);
- **arena**: `arena_size` bytes, vindo do limite de payload (ABI-17, padrão 1 MiB);
- região **batch** (offset 0, padrão 64 KiB, escrita pelo módulo) (ABI-18);
- região **answer** (offset `batch_size`, padrão 64 KiB, escrita pelo host) (ABI-19);
- região **frame-out** (escrita pelo módulo, um pedido remoto) (ABI-20) e **frame-in** (escrita pelo host, uma resposta) (ABI-21);
- arena na multi-memory é a memória 1, offsets absolutos (ABI-22); na single, região da memória 0 a partir de `pr_arena_base()` (ABI-23);
- o host descobre a variante por `pr_variant()` e pelo export `arena`, nunca por suposição (ABI-24);
- o host toma view da memória a cada uso e não a mantém entre chamadas (ABI-25).

**Imports (§3), namespace `"pr"`:**
- `flush(len)` — aplica os comandos do batch, em ordem; chamado no fim, na suspensão, no yield, quando o batch enche e antes de cada `query` (ABI-27);
- `query(len) -> i32` — batch com comandos e uma query; o host aplica, avalia, escreve a resposta na answer e devolve o tamanho ou erro negativo (ABI-28);
- `remote(len) -> i32` — envia o frame-out e devolve um request id (≥ 0) ou erro; a task então retorna `SUSPENDED` (ABI-29);
- `now() -> f64` (ms desde a época Unix, UTC) (ABI-30); `random() -> f64` em `[0, 1)` (ABI-31);
- `abort(len)` — fatal, com comando `ERROR`; o host reporta, encerra a task e mantém a aplicação; não retorna (ABI-32);
- imports só de conformance: `file_open`, `file_read`, `file_write`, `file_close` (ABI-33, detalhe em §8);
- um host que não fornece um import declarado se recusa a iniciar, com a mensagem de WASM-002 R15 (ABI-34).

**Exports (§4):**
- `memory` e `arena` (ABI-35); `pr_abi() -> i32` (valor `1`, ABI-36); `pr_variant() -> i32` (bit 0 = multi-memory, bit 1 = conformance) (ABI-37); `pr_arena_base() -> i32` (ABI-38); `pr_init() -> i32` (roda uma vez; `0` = ok) (ABI-39);
- `pr_open_form(form, opener) -> i32` — cria instância, devolve instance id ou erro (`-3` = sem instância livre, erro de WASM-017 R2 com o nome do form) (ABI-40);
- `pr_close_form(inst) -> i32` — handlers de fechamento, libera e reinicializa a região quando a task está `DONE` (ABI-41);
- `pr_event(inst, ctrl, event, argc) -> i32` — roda o handler ligado a `(ctrl, event)` com `argc` textos lidos da answer (ABI-42);
- `pr_resume(task, len) -> i32` — continua a task após `SUSPENDED` ou `YIELDED` (ABI-43);
- `pr_cancel(task)` — abandona a task suspensa (ABI-44);
- `pr_tick() -> i32` — task atual ou `-1` (ABI-45);
- `pr_status() -> i32` — status da execução mais recente (ABI-46);
- `pr_dbg_live()` e `pr_dbg_region_used()` — opcionais, para o teste de descarte (ABI-47, ABI-48);
- **status:** `0` DONE (ABI-49), `1` SUSPENDED (ABI-50), `2` YIELDED (ABI-51), `3` STOPPED — `STOP RUN` ou `GOBACK` do programa principal (ABI-52); negativo = erro (ABI-53);
- regra "um de cada vez" (ABI-54), substituída pela emenda A5.

**Batch, módulo → host (§5):**
- op `0x01` **SET** (`path`, `text`) — atribui valor a propriedade ou membro (ABI-55);
- op `0x02` **CALL** (`path`) — invoca método e descarta o resultado (ABI-56);
- op `0x03` **DISPLAY** (`text`) — console do host (ABI-57);
- op `0x04` **ERROR** (`varint code`, `text message`, `varint unit`, `varint line`, `varint form`, `varint event`) — erro com posição COBOL (ABI-58);
- op `0x05` **LOG** (`varint level`, `text`) — diagnóstico (ABI-59);
- gramática de `path`: `varint inst, varint ctrl, varint nseg, seg*`, onde `seg` é `varint name, varint nargs, text*` (ABI-60);
- ids de controle: `0` = a própria form (ABI-61); `1` = a form que abriu esta, `super::` (ABI-62); `2` = o objeto da aplicação, `COBOL::` (ABI-63); controles numerados a partir de `3`, na ordem de `forms[i].controls` (ABI-64);
- tabela `names` do manifest, compartilhada pelo módulo (ABI-65).

**Queries, módulo → host, síncronas (§6):**
- a região batch termina com uma query, op em `0x10…0x1F` (ABI-66);
- `0x10` **GET** (`path`), `0x11` **CALLR** (`path`), `0x12` **COUNT** (`path`) (ABI-67, ABI-68, ABI-69);
- o host aplica os comandos antes da query, para que a leitura reflita as escritas do próprio handler (ABI-70);
- o código gerado não emite query por iteração de laço, salvo leitura de controle no laço (ABI-71).

**Eventos, host → módulo (§7):**
- `pr_event` identifica o controle e o evento por índice na tabela `events` do manifest, com os nomes do desktop (ABI-72);
- `argc` textos no começo da answer (ABI-73);
- eventos sem handler não são entregues: o manifest lista os pares com handler (ABI-74).

**Conformance (§8), só hosts de teste:**
- `file_open(name_len, mode)` com modo 0 input, 1 output, 2 extend, 3 input/output (ABI-75); `file_read(handle, cap)`, `-1` = fim de arquivo (ABI-76); `file_write(handle, len)` (ABI-77); `file_close(handle)` (ABI-78); bit 1 de `pr_variant` ligado (ABI-79);
- o loader recusa módulo de conformance numa página, no shell Tauri e na preview (ABI-80).

**Códigos de erro (§9):** `-1` genérico (ABI-81); `-2` argumento inválido (ABI-82); `-3` sem instância livre (ABI-83); `-4` form, controle, evento ou verbo desconhecido (ABI-84); `-5` payload grande demais para a arena (ABI-85); `-6` host recusou por permissão ou política (ABI-86); `-7` não conectado ou falha de transporte (ABI-87); `-8` cancelado (ABI-88); `-9` timeout (ABI-89); `-10` módulo deu trap (ABI-90). Erro de execução COBOL não é trap: chega como `ERROR` e status negativo (ABI-91). Trap é bug do gerador e vira `-10` com a última linha conhecida, pelo global `$line` (ABI-92).

**Manifest (§10, JSON em `prmanifest`):** campos `abi` (ABI-93), `generator` (ABI-94), `project` (ABI-95), `variant` com `multi_memory` e `conformance` (ABI-96), `limits` com `arena_size` 1048576, `batch_size` 65536, `answer_size` 65536, `frame_size` 393216, `budget_blocks` 20000 e `data_pages` 17 (ABI-97), `names` (ABI-98), `events` (ABI-99), `forms` com `{id, name, main, instances, region_size, controls[{id,name,type}], handlers[{ctrl,event,entry}]}` (ABI-100), `builtins` (ABI-101), `imports` (ABI-102), `sizes` com `module`, `rt_functions` e `rt_bytes` (ABI-103). Custom sections `prmanifest` (ABI-104) e `prlines`, que liga um line id a unidade, linha e coluna (ABI-105). Global `$line` exportada para diagnóstico (ABI-106). Name section com `form.paragraph.block` (ABI-107).

**Checklist do host (§11):** ABI-108 (streaming, sonda de variante, checagem de `pr_abi`); ABI-109 (imports exatos, manifest, `pr_init`, `pr_open_form` do form principal); ABI-110 (aplicar o batch em ordem, nunca reter view, enfileirar eventos, retomar após `YIELDED`); ABI-111 (`pr_close_form` na destruição e soltar referências); ABI-112 (UTF-8 nas duas direções, uma conversão por direção).

**Emendas de 2026-10-20 (§12),** cada uma é regra que a implementação mostrou incompleta:
- **A1** — `pr_status` listado como export (ABI-114). Afeta o gerador: sim.
- **A2** — `pr_close_form` devolve status; região liberada quando `DONE` (ABI-115). Afeta o gerador só se o handler de fechamento suspender.
- **A3** — exports opcionais `pr_dbg_*` para o teste de descarte (ABI-116). Não afeta o gerador.
- **A4** — `ERROR.code` é a magnitude (`3` para `-3`) ou número COBOL a partir de `1000` (ABI-117). Afeta: sim.
- **A5** — **várias tasks suspensas ao mesmo tempo, uma por instância de form**; tasks de instâncias diferentes se intercalam (um `OpenFormSync` suspende o abridor enquanto o filho roda); eventos de uma mesma instância ficam em série; `pr_resume(task, len)` nomeia a task; `pr_cancel(-1)` descarta a execução que deu trap (ABI-118). Afeta: sim, tabela de tasks e estado por task. O fixture tem 4 tasks: o abridor, o filho modal e duas reservas.
- **A6** — **frame de erro** para chamada remota que falha: `{"id":N,"error":{"class","code","message","may_have_been_received"}}`, com classes `transport`, `authentication`, `limit` e `client`. Chamada enviada quando o link caiu tem `may_have_been_received: true` e nunca é repetida. Frame de sucesso: `{"id":N,"result":…}` (ABI-119). Afeta: sim.
- **A7** — verbos de `COBOL::` e `me::` que o player implementa, quais suspendem e o texto de argumento de cada evento (ABI-120). Verbo desconhecido responde `-4` e um `ERROR`. Afeta: sim.
- **A8** — envelope na conexão entre player e backend (ABI-121): primeira mensagem `{"type":"auth","token":…}`; chamada `{"type":"call","id":N,"frame":{…}}`; resposta `{"id":N,"result":…}` ou `{"id":N,"error":{…}}`; `ping` e `pong`; erro de autenticação com `expired` faz renovar o token e repetir **uma vez**. Ids abaixo de `0x80000000` são do módulo; acima, do player (controle Authentication). É **proposta do player** até WASM-014 substituí-la. Não afeta o gerador; o backend precisa falar isso.

**Verbos `COBOL::` (objeto da aplicação, ctrl 2) — §12.1:**
- `MessageBox`, `Alert` (texto, título, botões; suspende até fechar; botão em `DialogResult`) (ABI-122);
- `Confirm` (como o anterior, com botões de pergunta; suspende) (ABI-123);
- `InputBox` (texto, título, padrão; texto digitado em `DialogResult`; suspende) (ABI-124);
- `Sleep`, `Wait` (milissegundos; suspende) (ABI-125);
- `OpenFormSync`, `OpenFormAsync` (nome da form; handle; `Sync` suspende até o filho fechar) (ABI-126);
- `Navigate`, `OpenUrl` (endereço e alvo, pela política de WebLink; não suspende) (ABI-127);
- `Download` (nome e texto; não suspende) (ABI-128); `CopyToClipboard` (ABI-129); `SetTitle` (ABI-130);
- `GetProperty`, `SetProperty` (propriedades da aplicação: `DialogResult`, `Locale`, `Standalone`, `Title`…) (ABI-131);
- `StopRun`, `Exit` (aplicação termina; `STOPPED`) (ABI-132).

**Verbos `me::` (form, ctrl 0) — §12.1:**
- `OpenFormSync`, `OpenFormAsync` — suspende só o `Sync` (ABI-133);
- `Close` — fecha depois que o handler termina (ABI-134);
- `SetTitle`, `SetFullscreen`, `SetFocus`, `GetFormState`… (a lista termina em reticências, não é exaustiva) (ABI-135).

**Authentication (ctrl do controle) — §12.1:** `SignOut`, `Refresh`, `GetClaim` (valor de um claim, nunca o token) (ABI-136).

**Argumento de cada evento — §12.2 (todo argumento é `text`):**
- teclado (`onKeyDown`, `onKeyPress`, `onKeyUp`): a tecla (ABI-137);
- `onMouseWheel`: delta vertical (ABI-138); `onCheckedChanged`: `true` ou `false` (ABI-139); `onItemChecked`: itens marcados separados por line feed (ABI-140); `onItemDoubleClick`: texto do item (ABI-141); `onTabClick` e `onTabChanged`: número da página, a partir de 1 (ABI-142);
- `onNodeClick`, `onNodeSelect`, `onNodeDblClick`, `onNodeCheck`: `text⇥index⇥level⇥checked` (ABI-143);
- `onMenuClick`, `onMenuItemClick`: id do item (ABI-144); `onClick` de botão de ToolBar: id; de WebLink: modificadores (ABI-145);
- `onCellClick`, `onCellDoubleClick`: `row,column` a partir de **0** (ABI-146); `onRowDoubleClick`: linha; `onColumnClick`: coluna (a partir de 0);
- `onMapClick`: latitude e longitude a seis casas (ABI-147); `onMarkerClick`: id do marcador (ABI-148);
- `onFilesDropped`, `onFilesRejected`: nomes separados por line feed (ABI-149);
- `onButtonClick` do Snackbar: id do botão (ABI-150);
- `onSignedIn`, `onSignedOut`, `onSignUpCompleted`, `onPasswordResetRequested`, `onMfaRequired`: sem argumento (ABI-151);
- `onError` do Authentication: classe e código da resposta do backend, nunca a mensagem (ABI-152).

---

## 6. Plano de execução — WASM-PLAN (85 itens)

Documento de ordem, crates e risco. Cada spec tem o seu `plan.md` e `tasks.md`, escritos quando o spec for retomado. Não dá status por item: o estado real está no PROGRESS (seção 7).

**Artefatos e donos (§1):**
- **Program module** — `cobolt-wasm` (gerador e mini-linker), `cobolt-wasm-rt` (rotinas em Rust para `wasm32`, distribuídas como blob), `cobolt-decimal` (núcleo decimal exato compartilhado com o interpretador). Feito pelo Build da IDE, em segundos, sem toolchain Rust (specs 010, 017, 005);
- **Player** — `cobolt-web-player` (Leptos) e `cobolt-graphics`; construído uma vez pelo produto (specs 016, 004, 006, 007, 011, 018);
- **Host page e loader** — templates de `cobolt-compiler` (HTML, JS, CSS) (specs 002, 011, 013);
- **Backend** — `cobolt-server`, nativo (specs 003, 005, 014, 009);
- **Integração da IDE** — `cobolt-ide`, `cobolt-codegen`, `cobolt-project-tools` (specs 001, 008, 015, 012).

**Decisões de execução (P1–P8):**
- **P1** — `cobolt-wasm-rt` é `#![no_std]`, compilado para `wasm32-unknown-unknown`; rotinas escritas uma vez em Rust e ligadas estaticamente (WASM-010 Q3 (a)); blob versionado em `crates/cobolt-wasm/assets/rt.wasm` com hash lateral e teste de frescor; **mini-linker** copia só as funções alcançáveis (tree-shaking), renumera e funde os dados estáticos.
- **P2** — extração do núcleo decimal para `cobolt-decimal` (sem `std`, sem `alloc`), só em 2.x.x. Custo declarado: um fix 1.x.x em `value.rs` é portado à mão, não por cherry-pick.
- **P3** — o programa é rebaixado a um grafo de blocos; cada bloco é uma função que devolve o próximo id ou um código reservado (`SUSPEND`, `END`, `ERROR`). Nenhuma transferência de controle COBOL usa a pilha do WebAssembly. Estado de task (pc, pilha de `PERFORM`, pilha de `CALL`) na memória linear. Alternativas rejeitadas: função única com `loop`/`br_table`, Asyncify e JSPI.
- **P4** — descritor por item (offset, usage, digits, scale, sign, length); aritmética como sequência de `rt_ld`, `rt_add`, `rt_st` sobre pequena pilha de registradores num scratch que nunca atravessa suspensão.
- **P5** — **oráculo**: o interpretador, depois o padrão; decidem a suíte NIST e o padrão quando divergem. Toda diferença é classificada em `WASM-PARITY.md` (bug do gerador, bug do interpretador como fix 1.x.x, ou divergência aceita, com registro do texto do padrão). Harness diferencial `cobolt-wasm/tests/parity.rs`. O gate é a rodada NIST.
- **P6** — flag `--conformance` do gerador, para verbos de arquivo sequencial e line-sequential, só no gate NIST; esses verbos viram imports `file_*`; a flag fica no manifest; player e loader recusam módulo de conformance; o Build de projeto web nunca a liga.
- **P7** — engines de teste: `wasmi` (Rust puro) para unitários e conformance; `node` com V8 para a multi-memory fiel ao browser; Chrome e Safari pelas ferramentas de WASM-015 para o player.
- **P8** — ordem: GOLDEN RULE #9 vale para o gerador (NC termina nos dois eixos antes de IF e IC); o restante avança em paralelo onde os arquivos são disjuntos.

**Workstreams (W0–W8):**
- **W0** — documento ABI v1 e este plano (feito);
- **W1** — `cobolt-graphics`: display list, texto, ImageBuffer, Plot, Scene3D, Physics, handles, limits, dependency guard (spec 018; agente G, mergeado);
- **W2** — `CALL … WITH WEBSOCKET LINKAGE`, mapeamento group ⇄ JSON com golden vectors, `cobolt-server`, contrato e backend de identidade (specs 005, 003, 014, 009; agente B, em andamento);
- **W3** — `cobolt-decimal`, `cobolt-wasm-rt`, `cobolt-wasm` (layout, lowering, linker, glue da host API), harness de paridade, NC → IF → IC (specs 010, 017; feito em grande parte);
- **W4** — player: `cobolt-web-player`, JS do loader, host page, variantes, Viewer por construção de nós, controles classes A a E, CSS, navegação, WebLink, controle Authentication (lado frontend), controles Canvas ligados a W1 (specs 016, 002, 004, 006, 007, 011, 009, 018; agente P, mergeado em 2.0.24);
- **W5** — IDE: tipos de projeto e seletor, entradas da árvore, chaves `Tr`, ligação de Build, Run e Preview, preview em web view (specs 001, 008, 015; mergeado em 2.0.20);
- **W6** — portões de segurança e matriz de testes, ferramentas de agente, System KB e `chunked.data` (specs 013, 015; pendente);
- **W7** — shell mobile Tauri 2 (spec 012; pendente);
- **W8** — ledger de paridade e correções portadas ao 1.x.x onde o interpretador erra (spec 010 R16; `WASM-PARITY.md`; pendente).

**Riscos (1–7):**
1. O gerador é o caminho mais longo (NC = 95 programas, 4614 asserções). Mitigação: harness diferencial primeiro, fase A pequena, ledger por fase.
2. Layout fiel a bytes contra interpretador baseado em valores (`REDEFINES`, moves de grupo sobre `COMP-3`, `MOVE` sobreposto). O padrão e o NIST são o árbitro.
3. Mini-linker para módulo de Rust é infraestrutura nova (stack pointer global, data segments, tabela de chamadas indiretas, name section). Mitigação: `-C panic=abort`, sem `alloc`, censo de tree-shaking e `wasm-tools validate` em toda saída.
4. Toolchain: `trunk 0.22` pede rustc 1.96.1 (instalado: 1.95). Usar `trunk 0.21.14` ou `wasm-bindgen` direto; nightly fixado para o threaded, instalado quando W4 chegar lá.
5. `lopdf` impede `cobolt-forms` de compilar para `wasm32` (código de PDF do Viewer). Desligar o PDF em `wasm32` por feature do cargo, o que W4 faz e testa nos dois alvos.
6. Disco: cada worktree tem seu `target/`; agentes clonam com `cp -cRp`; nenhum `cargo clean` do compartilhado.
7. Agentes em paralelo fundindo: cada agente é dono de crates disjuntos; a integração é por merge em `features-2x`, resolvendo `Cargo.toml`, `Cargo.lock`, versão e CHANGELOG à mão.

**Definição de pronto da série (PLAN §5):** cada spec em **V** no ledger, com evidência nomeada, e cinco fatos verdadeiros, cada um com rodada registrada:
1. um projeto web construído pelo Build da IDE abre no navegador e na Preview da IDE; handlers rodam no módulo gerado; controles são desenhados pelo player;
2. um `CALL` remoto chega a um backend autenticado por `wss`, suspende o módulo e retoma com a resposta;
3. NC, IF e IC rodam no módulo gerado com 0 falhas;
4. os players threaded e plain e as duas variantes do módulo passam a matriz de WASM-017 R20;
5. os shells Android e iOS rodam o mesmo bundle conforme o toolchain da máquina; o que não puder rodar fica no ledger com o comando exato, "nunca declarado como feito".

---

## 7. Estado de implementação (WASM-PROGRESS.md, 2026-10-20)

Legenda do ledger: **S** especificado · **P** planejado (plan.md) · **T** com tasks (tasks.md) · **I** implementando · **V** verificado. Uma linha só fica **done** quando seus critérios de aceitação rodaram e passaram, com evidência nomeada.

**7.1 Status no ledger:** as 18 specs estão marcadas **só como S**. As colunas P, T, I e V estão vazias. O texto do próprio ledger registra trabalho bem além disso (ver 7.2), então a tabela está desatualizada.

**7.2 Já feito**
- **NC (NIST Nucleus) em módulo gerado** — 2.0.19: compila 95/95 e roda 95/95 com 0 falhas. Módulo: 4612 PASS / 0 FAIL / 5 DELETED; interpretador: 4614 / 0 / 5. A diferença de 2 é a contagem do harness (conta marcadores `PASS` no print file), e a linha-resumo de cada programa é idêntica nos 95. Doze mudanças de interpretador aplicadas e testadas (comparação de número com caracteres, `BLANK WHEN ZERO`, `VALUE` em editado, move de grupo alfanumérico-editado, de-edição com inserções, `INSPECT` em numérico com sinal, `OCCURS DEPENDING ON`, nível 66 e `RENAMES`, `PERFORM VARYING AFTER`, `WHEN` empilhados, `DIVIDE GIVING` editado, chaves externas via host).
- **IF (funções intrínsecas) em módulo gerado** — 2.0.20: 45/45 compila e roda; 841 PASS / 0 FAIL.
- **IC (programas aninhados e chamadas)** — 2.0.22: 25/25 compila e roda; 309 PASS / 0 FAIL / 4 DELETED. Cobre unidades de programa, `CALL`/`CANCEL`, LINKAGE, `GLOBAL`/`EXTERNAL`, arquivos `OPTIONAL` e declaratives `USE`.
- **`cobolt-decimal`** — V. Suíte do interpretador: 1527 passam, 0 falham.
- **`cobolt-numedit`** — V. 199.233 casos aleatórios, 0 diferenças contra o original.
- **`cobolt-wasm-rt`** — I. 15 testes nativos (codecs, MOVE, aritmética, condições, DISPLAY, pictures editadas, INSPECT, STRING, UNSTRING, INITIALIZE, arquivos de conformance).
- **`cobolt-wasm` (gerador), fases A e B de WASM-010** — I. Programas iguais ao interpretador, byte a byte, nas duas variantes de memória. Módulos validam. "Hello" tem 15,8 KB.
- **Player, loader, host page e bundle** — mergeados em 2.0.24 (7 commits). Verificados em Chrome e node contra um **módulo fixture**, não contra um módulo gerado.
- **ABI A5 a A7** — vinculam o gerador: os exports de form em `compile_project` são escritos contra elas (PROG-43, vinculante).
- **IDE: tipos de projeto, seletor, árvore, Preview e ferramentas** — mergeados em 2.0.20.
- **`cobolt-graphics`, parte 1 (o crate)** — mergeado em 2.0.21. Conforme o plano: 330 testes passam no macOS; compila para `wasm32` em 20 de 20 combinações de features.
- **`CALL … RETURNING` local** — fix `8a15f00d` (1.91.219) em `main`, portado como `afe22c51` (2.0.1) em 2.x.x.

**7.3 Pendente**

*Gerador (WASM-010, WASM-017)*
- Exports de form em `module.rs`: tabela de instâncias, `pr_open_form`, `pr_event`, `pr_resume`, `pr_cancel`, `pr_tick`, `pr_close_form`, `pr_status`, glue de `query` e `random`, e `forms`, `names`, `events` no manifest. É o próximo passo declarado no ledger (item 1 de "Where the work stopped").
- Métodos de valor em itens de dados (`Trim`, `Upper`/`UpperCase`, `Lower`, `Replace`, `Len`, `Split`). Hoje são recusados como `Expr::Member` sobre raiz de dado.
- Host de teste que dirige forms, com teste de clique de botão (item 3).
- **Fase C** (suspensão, `CALL` remoto, glue de `remote`, mapeamento JSON) — não iniciada no registro (PROG-77).
- **Fase D** (resto do subconjunto).
- Ponta a ponta: Build da IDE → bundle → player (item 6).
- Regressão NIST do interpretador para os campos de AST aditivos (`FileDescription::is_external`, `UseProcedure::is_global`) (item 7).

*Backend (WASM-003, 005, 009, 014)*
- Agente de backend em andamento, **não mergeado** (branch `worktree-agent-a24ab0673830285c1`). Inclui `CALL … WITH WEBSOCKET LINKAGE`, JSON, `cobolt-server` e o backend de identidade (PROG-68, PROG-79).

*Player (WASM-016, 002, 004, 006, 007, 011)*
- Planos e tarefas (`plan.md`, `tasks.md`) dos specs 016, 002, 004, 006, 007 e 011 ainda não existem (PROG-45).
- Variante **threaded** não rodou num navegador: ordenação Rayon do DataGrid (PROG-46).
- Push de rota do shell ainda não conferido no Chrome (PROG-47).
- Avisos de dead-code (PROG-48).
- WASM-016 AC7: freshness do player entregue (PROG-49).
- Células da matriz de navegadores de WASM-015 R20 que não puderam ser medidas (PROG-50).
- **Leptos:** o uso do Leptos no player está **sem decisão** do operador (PROG-44; ver seção 9).

*IDE e Preview (WASM-001, 008, 015)*
- Filtro da toolbox por kind (WASM-001 R14), à espera da tabela de classes de WASM-004.
- Scaffold do backend: só o `HEALTH` é criado (WASM-001 R6). Os programas de autenticação dependem de WASM-009.
- Opções `[desktop]` (window style, UI framework, High DPI, GPU) são gravadas, mas não têm efeito em execução.
- Target platform do Desktop mostra o host, sem escolha (Q4).
- WASM-008 T11–T13: abrir uma janela real uma vez (pelo operador); ligar o gerador real (`web::link`) e medir AC1–AC2; backend iniciado sob pedido (R6); render headless e execução de form web (R9, depende de WASM-015 R1–R2).
- WASM-008 não cumprido: R3 é reload, não atualização in-place; erros não são pintados sobre a página (vão ao Output); emulação é só no nível de script (`@media (resolution)` e `env(safe-area-inset-*)` não mudam); Windows e Linux não testados.

*Gráficos (WASM-018)*
- **Partes 2 e 3 não iniciadas:** T11 a T22 (listadas na seção 3, WASM-018, Plano).
- Não rodados: AC1, AC12, AC13 e AC14. AC4, AC5, AC6 e AC7 só em parte.
- Build threaded de `cobolt-graphics`: só type-checked.
- System KB e `chunked.data`: não atualizados para os controles novos (os métodos existem no crate, mas não nas tabelas de documentação do compilador).
- Rótulos em seis idiomas, controles Canvas2D e Canvas3D, gamepad e `examples/PowerGraphics`: não feitos.
- 26 avisos de estilo do clippy na biblioteca.

*Não iniciados (PROG-82 a PROG-85)*
- Shell Tauri (WASM-012).
- Portões de segurança (WASM-013).
- Integração do controle Canvas (WASM-018 parte 2).
- Refresh de `CLAUDE.md` e da memória.

*Specs sem entrada própria de execução no ledger:* WASM-004, 006, 007, 011 (têm trabalho previsto no player, ainda não iniciado). WASM-009 e WASM-014 (têm trabalho no backend, não mergeado). WASM-015 (parte da KB e das regras do plugin foi feita para project kinds e Preview, segundo WASM-001 e WASM-008).

---

## 8. Divergências entre o gerador e o interpretador (WASM-PARITY.md)

Regra do ledger: toda diferença é registrada, com quem está certo. Um defeito do **interpretador** vira correção em `main` (GOLDEN RULE #5), portada para 2.x.x e listada até o operador decidir. Uma diferença deliberada fica registrada com o motivo.

| # | Assunto | Módulo gerado | Interpretador | Quem está certo | Status |
|---|---|---|---|---|---|
| 1 | `IF N = ZERO` com N numérico `BLANK WHEN ZERO` | verdadeiro | falso (compara texto em branco) | módulo (COBOL-85 5.4) | **aberta: reportar ao operador**. Correção 1.x.x em `Interpreter::as_comparand`. Nenhum membro do NIST exercita |
| 2 | Numérico-editado `BLANK WHEN ZERO` com valor que vira zero só após truncar (`0.001` em `Z9.99`) | brancos | imprime `0.00` | módulo | aberta, mesma família de #1 |
| 3 | Arquivo sequencial no build de conformance: um registro por linha ou tamanho fixo sem separador | um por linha | tamanho fixo | nenhum; escolha do test host | **by design**. Por isso 4612 PASS contra 4614 |
| 4 | `FUNCTION`, programas aninhados, `CALL … LINKAGE`, `EXEC`, membros de controle | recusados no build | rodam | interpretador | aberta. IF e IC já cobrem `FUNCTION`, aninhados e `LINKAGE`; `EXEC` e membros de controle seguem sem tratamento |
| 5 | `FUNCTION WHEN-COMPILED` | hora do build (constante) | hora da execução | módulo (COBOL-85) | **aberta: reportar ao operador**. Correção 1.x.x |
| 6 | `CURRENT-DATE` com offset e `ACCEPT … FROM DATE/TIME` | UTC, `+0000` | relógio local | por desenho: o navegador tem o relógio do host | **by design** |
| 7 | `FUNCTION MAX/MIN/SUM` sobre tabela `OCCURS … DEPENDING ON` | recusado no build | expande pela contagem atual | interpretador | aberta. Precisa de contagem em tempo de execução no registro de chamada |
| 8 | `EXIT PROGRAM` ou `GOBACK` dentro de um `PERFORM` em programa chamado | sai e volta ao chamador (COBOL-85 6.7.4) | segue depois do `PERFORM` | módulo | **aberta: reportar ao operador**. Correção 1.x.x, achada por `calls.rs::a_callee_that_returns_from_inside_a_perform` |
| 9 | `CALL … RETURNING` com identificador como nome, e `BY VALUE` | ignora `RETURNING` em chamada dinâmica; `BY VALUE` como `BY CONTENT` | certo | interpretador | aberta. Precisa do callee conhecido no ponto da chamada, e do `USAGE` do callee para `BY VALUE` |
| 10 | `PROGRAM-ID … INITIAL` ou `COMMON`; recursão; `CALL "COBOL-…"` de serviço | não gerado | alguns rodam | nenhum | aberta |

Resumo: **by design**: #3 e #6. **Aguardam aval do operador** (handoff §7): #1, #2, #5 e #8, que são correções 1.x.x. **Abertas simples**: #4, #7, #9 e #10.

---

## 9. Decisões e pendências que dependem do operador

**9.1 Decisões de WASM-INDEX §3 (D1 a D19)**

| # | Decisão | Status |
|---|---|---|
| D1 | Modelo de execução: o compilador gera WebAssembly; sem interpretador nem runtime no browser; sem `redb` nem disco no browser; o backend mantém o runtime | **decidida** (2026-10-19; threads liberadas em D16) |
| D1b | Geração direta, ou via Rust | **decidida e confirmada** (2026-10-20) |
| D2 | Senha digitada na nossa form ou no provedor | **decidida**: o fluxo é por provedor. Falta fixar o padrão de cada adaptador |
| D3 | Precedência de CSS | **decidida**: CSS vence |
| D4 | Backend com chamadas stateless ou stateful | aberta (WASM-003 Q2) |
| D5 | WebSocket apenas, ou também uma pequena superfície HTTPS | decidida em parte: HTTPS mínimo; detalhes abertos |
| D6 | `RETURNING` em `CALL` local | **decidida e feita** (fix em 1.x.x, portado) |
| D7 | Preview em processo separado ou embutida | **decidida no plano**: processo separado (WASM-008 Q1) |
| D8 | Layout na web: CSS ou o solver existente | **decidida**: CSS |
| D9 | Estilo de aplicação: SPA por padrão, standalone como alternativa | **decidida** |
| D10 | Player: dois `.wasm` (o módulo e o player Leptos prebuilt) | **decidida**. Custo aceito: sem código Leptos por projeto |
| D11 | Paridade do modelo de dados (bytes do gerador, valores do interpretador) | aberta (WASM-010 Q5) |
| D12 | Templates criáveis na 2.0: Web, Android, iOS; sem watch | **decidida** |
| D13 | Backend é parte de todo projeto web ou mobile, não kind | **decidida** (2026-10-20) |
| D14 | Markup nunca é inserido como string HTML; WebLink sem `ping` na v1 | **decidida** (2026-10-20) |
| D15 | Arquitetura: um módulo por projeto, regiões por instância, data memory fixa, arena de transporte, granularidade grossa | **decidida** (2026-10-20) |
| D16 | Threads e Rayon na v1, com isolamento cross-origin e fallback plain | **decidida** (2026-10-20), revertendo o ruling de 2026-10-19 |
| D17 | Multi-memory padrão, com variante single-memory para iOS e matrizes com Safari | **decidida** (2026-10-20) |
| D18 | Component Model e ES-module integration: não mantidos, por falta de suporte do Chrome | **decidida** (2026-10-20) |
| D19 | Canvas 2D, Canvas 3D e cinco objetos em todas as plataformas, em crates 100 % Rust | **decidida** (2026-10-20). Versões fixadas no `/plan` |

**9.2 Pendências de decisão do operador**

1. **Leptos no player** (PROG-44; WASM-016 R1 e Q2). O plano de W4 usa Leptos, mas o ledger mostra que o agente não o usou. O desvio fica atrás do trait `Surface`. Precisa de decisão.
2. **Merge de `features-2x` em `2.x.x`**: só quando o operador pedir (handoff §7).
3. **Correções 1.x.x de paridade** #1, #2, #5 e #8 (seção 8).
4. **Pentest externo** (WASM-013 Q1): obrigatório antes do 2.0, ou revisão por segundo leitor?
5. **Contato e política de segurança** (WASM-013 Q4).
6. **Token de desenvolvimento** (WASM-003 Q5): só em execução de desenvolvimento, ou nunca?
7. **Primeiros recursos de dispositivo** (WASM-012 Q1): camera, location, notifications, biometria ou leitor de código de barras, além de secure storage, abrir endereço e deep links?
8. **Tipos de MFA no 2.0** (WASM-009 Q5): só TOTP ou também SMS e e-mail?
9. **Quanta diferença visual entre web e desktop é aceitável** (WASM-004 Q6 e WASM-007 Q5; INDEX §5, lacuna 1).
10. **Adaptador de software para CI** dos goldens 3D (WASM-018 Q5).
11. **Revisão externa antes do release candidate** (WASM-013 R16).
12. **Uma janela real pelo operador** para a Preview (WASM-008 T11).

---

## 10. Contradições e lacunas entre os documentos

1. **Leptos.** O plano (W4) usa Leptos no `cobolt-web-player`. O ledger diz que Leptos não é usado (WASM-016 R1 e Q2) e pede decisão do operador.
2. **Divergência aceita.** O plano (P5) proíbe documentar a diferença para fazê-la sumir (R16c). O PARITY tem duas linhas "by design" (#3 e #6), e o preâmbulo diz que a diferença deliberada fica registrada.
3. **Estado dos agentes.** O plano dá os agentes G e B como "running". O ledger dá G mergeado (2.0.21) e P mergeado (2.0.24). B segue sem merge.
4. **PARITY #4** diz "open: IF and IC modules", mas o ledger dá IF 45/45 e IC 25/25 como terminados.
5. **O ledger está desatualizado.** A tabela "Status by spec" está só em S, enquanto o texto registra trabalho V e terminado. Seções antigas dizem "Next: IF, then IC" e "IF, IC not started", já superadas.
6. **ABI §4 "One task at a time"** contradiz a emenda A5. Vale a A5.
7. **WASM-PLAN §1** diz "four artefacts", mas a tabela tem cinco linhas.
8. **WASM-PARITY.md** não tem o cabeçalho SPDX/Copyright dos outros arquivos. Diferença de forma, não de conteúdo.
9. **Status dos specs e dos planos.** Todos os 18 specs estão como "rascunho, precisa de `/clarify`". Mas WASM-001 e WASM-008 têm plan.md e tasks.md dizendo "implementado" ou "em andamento". Os specs não foram atualizados.
10. **WASM-001 R5 contra o non-goal.** O non-goal fala só dos watches e das duas entradas Web como "ainda não criáveis". R5 diz que as criáveis são as seis listadas, o que torna também indisponíveis Chatbot (desktop) e UserControl (Desktop).
11. **WASM-018 Q4 e Q7.** O texto de R16 e do §1 chama a questão de renderizador desktop de "Q7". O §7 a numera como Q4.
12. **WASM-013 R15b reverte o ruling anterior** ("threads não são necessárias no navegador"), de 2026-10-19. A reversão é intencional e está registrada em D16.
13. **WASM-PARITY e PROGRESS** indicam que o contrato de backend (A8) é proposta do player, e que WASM-014 ainda precisa ratificá-la. Enquanto isso, o backend não foi mergeado.
14. **Lacunas que nenhum spec cobre**: suporte a `ACCEPT` no browser (WASM-010 Q9 aberta); uma stylesheet de projeto inteiro (WASM-007 Q3, deixada para depois); e a extensibilidade do player (controles `Custom` e external crates na web: WASM-016 Q5, WASM-032).

---

*Fim do inventário. Cada item acima vem de um requisito R, de um Q ou de um item de plano, e a origem está indicada. O arquivo não é commitado: é um documento de trabalho na worktree.*

## 11. Adendo 2.0.26 — camadas de formulário (spec 091) adaptadas para HTML

A série WASM não tinha as camadas do desktop (spec 091). Foram acrescentadas como regras que valem para formulários web sem mudança, e como a realização em HTML de cada regra. O total de requisitos R passa de 300 para **322** (+22). Entram também a emenda A9 do contrato ABI, a decisão D20 (proposta) e a linha W4 do plano.

- **WASM-004 R13–R24 (12):** modelo de camadas e regras de design herdadas; empilhamento por contexto de empilhamento; visibilidade (oculta por `display: none` e `inert`, começa oculta, sem estado `Visible` na descrição); ponteiro pelo teste de acerto do navegador (fundo transparente deixa passar, fundo opaco descarta); recorte por `overflow: hidden` com o raio do formulário, inclusive os sete tipos que o desktop não recorta; fundo da camada no próprio elemento, com `Transparency` como alfa; layout só na base; ordem de tabulação pela ordem do DOM, sem `tabindex` positivo; conjunto de não visuais igual ao predicado do catálogo; propriedades de camada em tempo de execução, por nome; camadas em ContentPane e janelas standalone; formulário sem camadas inalterado.
- **WASM-007 R15–R16 (2):** a raiz da camada como contexto de empilhamento; regras de folha de estilo que não podem mudar empilhamento nem visibilidade.
- **WASM-008 R11 (1):** a Preview começa com todas as camadas ocultas.
- **WASM-010 R19–R20 (2):** propriedades de camada como `LAYER-NAME::Property`; erro de build para nome desconhecido; camada sem handler de evento próprio.
- **WASM-011 R16 (1):** formulário com camadas no ContentPane e em janela standalone.
- **WASM-015 R17–R18 (2):** guard de paridade com um formulário de referência com camadas; KB e regras do plugin.
- **WASM-016 R18–R19 (2):** descrição do formulário com camadas; estado de visibilidade por instância, oculto ao criar e liberado com a instância.
- **Contrato ABI, amendment A9:** camadas como objetos endereçáveis, ids depois dos controles, propriedades por nome e `Visible` como estado por instância.
- **Índice:** decisão D20 (proposta) e linha na tabela de itens do operador. **Plano:** W4.
- **Questões novas:** WASM-004 Q7 (opacidade do fundo), Q8 (toast do Snackbar) e Q9 (área de toque).

Todas as escolhas de realização em HTML (`inert`, `pointer-events`, `overflow: hidden`, ordem do DOM) são propostas e ficam para confirmar no `/clarify`.

## 12. Adendo 2.0.27 — decisões de 2026-10-20 na WASM-001

- **Q1, Q4, Q5, Q7 e Q8:** aceitas como recomendadas.
- **Q2:** forms web com `.wcfrm`, forms mobile com `.mcfrm`, desktop com `.cfrm`. Refletido em WASM-004 R1 e R13, WASM-007 (lugar da stylesheet) e WASM-016 R3. A implementação ainda escreve `.cfrm` para web e mobile: tarefa T15 aberta.
- **Q6:** *Create from Template…* copia um projeto salvo. *Recent Projects* entra no rodapé, com a confirmação pendente: o código já tem uma lista de recentes no diálogo (`new_project.rs`, alimentada por `ui_prefs` em `app.rs`).
- **R7 da WASM-001** virou requisito (antes era opcional).
