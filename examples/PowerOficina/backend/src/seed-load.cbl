       IDENTIFICATION DIVISION.
       PROGRAM-ID. SEED-LOAD.
      *> PowerOficina - loads the starting data: the catalog (parts, services, add-ons and the labour rate)
      *> with its own prices in BRL and in EUR, the fault rules and the number sequences.
      *> The data is fictitious. Nothing is converted between currencies: each row carries the price of its own.
      *> Safe to run again: a row that already exists is kept as it is and counted as skipped.
      *> Not exported, so a browser cannot call it. Run it from the project folder: rcrun run backend/src/seed-load.cbl

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CATALOG-FILE ASSIGN TO "data/catalog.idx"
               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
               RECORD KEY IS CAT-KEY FILE STATUS IS WS-FS-CAT.
           SELECT FAULT-RULE-FILE ASSIGN TO "data/faultrules.idx"
               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
               RECORD KEY IS FLT-CODE FILE STATUS IS WS-FS-FLT.
           SELECT SEQUENCE-FILE ASSIGN TO "data/sequences.idx"
               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
               RECORD KEY IS SEQ-NAME FILE STATUS IS WS-FS-SEQ.

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/catalog.FD".
           COPY "COPYBOOKS/faultrules.FD".
           COPY "COPYBOOKS/sequences.FD".

       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/ws-partiql".
       01 WS-FS-CAT                              PIC    XX.
       01 WS-FS-FLT                              PIC    XX.
       01 WS-FS-SEQ                              PIC    XX.
       01 WS-I                                   PIC    999.
       01 WS-CUR                                 PIC    9.
       01 WS-CAT-NEW                             PIC    9(4)    VALUE 0.
       01 WS-CAT-SKIP                            PIC    9(4)    VALUE 0.
       01 WS-FLT-NEW                             PIC    9(4)    VALUE 0.
       01 WS-FLT-SKIP                            PIC    9(4)    VALUE 0.
       01 WS-SEQ-NEW                             PIC    9(4)    VALUE 0.
       01 WS-SEQ-SKIP                            PIC    9(4)    VALUE 0.
       01 WS-FAIL                                PIC    9(4)    VALUE 0.

      *> Catalog rows: code, kind (P part, S service, A add-on, L labour), name, standard hours, BRL price, EUR price.
       01 CAT-DATA.
           05 FILLER PIC X(12) VALUE "PAD-FREIO".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Pastilha de freio dianteira".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 180.00.
           05 FILLER PIC 9(7)V99 VALUE 42.00.
           05 FILLER PIC X(12) VALUE "DISCO-FREIO".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Disco de freio dianteiro (par)".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 420.00.
           05 FILLER PIC 9(7)V99 VALUE 96.00.
           05 FILLER PIC X(12) VALUE "VELA".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Vela de ignição (jogo de 4)".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 160.00.
           05 FILLER PIC 9(7)V99 VALUE 38.00.
           05 FILLER PIC X(12) VALUE "BOBINA".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Bobina de ignição".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 230.00.
           05 FILLER PIC 9(7)V99 VALUE 54.00.
           05 FILLER PIC X(12) VALUE "FILTRO-COMB".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Filtro de combustível".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 75.00.
           05 FILLER PIC 9(7)V99 VALUE 19.00.
           05 FILLER PIC X(12) VALUE "FILTRO-AR".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Filtro de ar do motor".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 55.00.
           05 FILLER PIC 9(7)V99 VALUE 14.00.
           05 FILLER PIC X(12) VALUE "FILTRO-OLEO".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Filtro de óleo".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 38.00.
           05 FILLER PIC 9(7)V99 VALUE 9.00.
           05 FILLER PIC X(12) VALUE "FILTRO-CAB".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Filtro de cabine".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 62.00.
           05 FILLER PIC 9(7)V99 VALUE 16.00.
           05 FILLER PIC X(12) VALUE "OLEO-5W30".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Óleo 5W30 sintético (litro)".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 48.00.
           05 FILLER PIC 9(7)V99 VALUE 11.00.
           05 FILLER PIC X(12) VALUE "SENSOR-MAP".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Sensor de pressão do coletor".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 210.00.
           05 FILLER PIC 9(7)V99 VALUE 49.00.
           05 FILLER PIC X(12) VALUE "SENSOR-O2".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Sonda lambda".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 340.00.
           05 FILLER PIC 9(7)V99 VALUE 79.00.
           05 FILLER PIC X(12) VALUE "SENSOR-ROT".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Sensor de rotação".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 190.00.
           05 FILLER PIC 9(7)V99 VALUE 45.00.
           05 FILLER PIC X(12) VALUE "CATALISADOR".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Catalisador".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 1850.00.
           05 FILLER PIC 9(7)V99 VALUE 430.00.
           05 FILLER PIC X(12) VALUE "BATERIA".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Bateria 60Ah".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 620.00.
           05 FILLER PIC 9(7)V99 VALUE 138.00.
           05 FILLER PIC X(12) VALUE "ALTERNADOR".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Alternador".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 980.00.
           05 FILLER PIC 9(7)V99 VALUE 225.00.
           05 FILLER PIC X(12) VALUE "TERMOSTATO".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Válvula termostática".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 95.00.
           05 FILLER PIC 9(7)V99 VALUE 23.00.
           05 FILLER PIC X(12) VALUE "BOMBA-AGUA".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Bomba d'água".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 310.00.
           05 FILLER PIC 9(7)V99 VALUE 72.00.
           05 FILLER PIC X(12) VALUE "VALVULA-EGR".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Válvula EGR".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 720.00.
           05 FILLER PIC 9(7)V99 VALUE 166.00.
           05 FILLER PIC X(12) VALUE "INJETOR".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Bico injetor".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 380.00.
           05 FILLER PIC 9(7)V99 VALUE 88.00.
           05 FILLER PIC X(12) VALUE "CORREIA-DENT".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Kit correia dentada".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 540.00.
           05 FILLER PIC 9(7)V99 VALUE 124.00.
           05 FILLER PIC X(12) VALUE "AMORTECEDOR".
           05 FILLER PIC X(1) VALUE "P".
           05 FILLER PIC X(40) VALUE "Amortecedor dianteiro (par)".
           05 FILLER PIC 9(3)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 860.00.
           05 FILLER PIC 9(7)V99 VALUE 198.00.
           05 FILLER PIC X(12) VALUE "SVC-FREIO".
           05 FILLER PIC X(1) VALUE "S".
           05 FILLER PIC X(40) VALUE "Serviço: sistema de freios".
           05 FILLER PIC 9(3)V99 VALUE 1.5.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC X(12) VALUE "SVC-IGNICAO".
           05 FILLER PIC X(1) VALUE "S".
           05 FILLER PIC X(40) VALUE "Serviço: sistema de ignição".
           05 FILLER PIC 9(3)V99 VALUE 1.0.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC X(12) VALUE "SVC-ADMISSAO".
           05 FILLER PIC X(1) VALUE "S".
           05 FILLER PIC X(40) VALUE "Serviço: sistema de admissão".
           05 FILLER PIC 9(3)V99 VALUE 1.0.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC X(12) VALUE "SVC-ESCAPE".
           05 FILLER PIC X(1) VALUE "S".
           05 FILLER PIC X(40) VALUE "Serviço: sistema de escape".
           05 FILLER PIC 9(3)V99 VALUE 2.5.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC X(12) VALUE "SVC-BATERIA".
           05 FILLER PIC X(1) VALUE "S".
           05 FILLER PIC X(40) VALUE "Serviço: bateria e carga".
           05 FILLER PIC 9(3)V99 VALUE 0.5.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC X(12) VALUE "SVC-ARREFEC".
           05 FILLER PIC X(1) VALUE "S".
           05 FILLER PIC X(40) VALUE "Serviço: arrefecimento".
           05 FILLER PIC 9(3)V99 VALUE 2.5.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC X(12) VALUE "SVC-EGR".
           05 FILLER PIC X(1) VALUE "S".
           05 FILLER PIC X(40) VALUE "Serviço: válvula EGR".
           05 FILLER PIC 9(3)V99 VALUE 1.5.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC X(12) VALUE "SVC-INJECAO".
           05 FILLER PIC X(1) VALUE "S".
           05 FILLER PIC X(40) VALUE "Serviço: injeção de combustível".
           05 FILLER PIC 9(3)V99 VALUE 2.0.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC X(12) VALUE "SVC-CORREIA".
           05 FILLER PIC X(1) VALUE "S".
           05 FILLER PIC X(40) VALUE "Serviço: correia dentada".
           05 FILLER PIC 9(3)V99 VALUE 3.0.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC X(12) VALUE "SVC-SUSP".
           05 FILLER PIC X(1) VALUE "S".
           05 FILLER PIC X(40) VALUE "Serviço: suspensão".
           05 FILLER PIC 9(3)V99 VALUE 2.0.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC 9(7)V99 VALUE 0.00.
           05 FILLER PIC X(12) VALUE "ADD-CARPETE".
           05 FILLER PIC X(1) VALUE "A".
           05 FILLER PIC X(40) VALUE "Limpeza de carpetes".
           05 FILLER PIC 9(3)V99 VALUE 0.8.
           05 FILLER PIC 9(7)V99 VALUE 120.00.
           05 FILLER PIC 9(7)V99 VALUE 28.00.
           05 FILLER PIC X(12) VALUE "ADD-FILTROS".
           05 FILLER PIC X(1) VALUE "A".
           05 FILLER PIC X(40) VALUE "Troca de filtros (ar e cabine)".
           05 FILLER PIC 9(3)V99 VALUE 0.5.
           05 FILLER PIC 9(7)V99 VALUE 90.00.
           05 FILLER PIC 9(7)V99 VALUE 21.00.
           05 FILLER PIC X(12) VALUE "ADD-OLEO".
           05 FILLER PIC X(1) VALUE "A".
           05 FILLER PIC X(40) VALUE "Troca de óleo e filtro".
           05 FILLER PIC 9(3)V99 VALUE 0.7.
           05 FILLER PIC 9(7)V99 VALUE 110.00.
           05 FILLER PIC 9(7)V99 VALUE 26.00.
           05 FILLER PIC X(12) VALUE "ADD-LAVAGEM".
           05 FILLER PIC X(1) VALUE "A".
           05 FILLER PIC X(40) VALUE "Lavagem geral".
           05 FILLER PIC 9(3)V99 VALUE 1.0.
           05 FILLER PIC 9(7)V99 VALUE 80.00.
           05 FILLER PIC 9(7)V99 VALUE 19.00.
           05 FILLER PIC X(12) VALUE "HORA-MO".
           05 FILLER PIC X(1) VALUE "L".
           05 FILLER PIC X(40) VALUE "Mão de obra (hora)".
           05 FILLER PIC 9(3)V99 VALUE 1.0.
           05 FILLER PIC 9(7)V99 VALUE 150.00.
           05 FILLER PIC 9(7)V99 VALUE 45.00.
       01 CAT-TABLE REDEFINES CAT-DATA.
           05 CAT-ROW OCCURS 36 TIMES.
               10 CR-CODE                        PIC X(12).
               10 CR-KIND                        PIC X(1).
               10 CR-NAME                        PIC X(40).
               10 CR-HOURS                       PIC 9(3)V99.
               10 CR-BRL                         PIC 9(7)V99.
               10 CR-EUR                         PIC 9(7)V99.

      *> Fault rules: code, description, three part codes, one service code.
       01 FLT-DATA.
           05 FILLER PIC X(8) VALUE "P0301".
           05 FILLER PIC X(60) VALUE "Falha de ignição no cilindro 1".
           05 FILLER PIC X(12) VALUE "VELA".
           05 FILLER PIC X(12) VALUE "BOBINA".
           05 FILLER PIC X(12) VALUE SPACES.
           05 FILLER PIC X(12) VALUE "SVC-IGNICAO".
           05 FILLER PIC X(8) VALUE "P0300".
           05 FILLER PIC X(60) VALUE "Falhas de ignição aleatórias".
           05 FILLER PIC X(12) VALUE "VELA".
           05 FILLER PIC X(12) VALUE "BOBINA".
           05 FILLER PIC X(12) VALUE "FILTRO-COMB".
           05 FILLER PIC X(12) VALUE "SVC-IGNICAO".
           05 FILLER PIC X(8) VALUE "P0171".
           05 FILLER PIC X(60) VALUE "Mistura pobre (banco 1)".
           05 FILLER PIC X(12) VALUE "SENSOR-MAP".
           05 FILLER PIC X(12) VALUE "FILTRO-AR".
           05 FILLER PIC X(12) VALUE SPACES.
           05 FILLER PIC X(12) VALUE "SVC-ADMISSAO".
           05 FILLER PIC X(8) VALUE "P0420".
           05 FILLER PIC X(60) VALUE "Eficiência do catalisador baixa".
           05 FILLER PIC X(12) VALUE "CATALISADOR".
           05 FILLER PIC X(12) VALUE "SENSOR-O2".
           05 FILLER PIC X(12) VALUE SPACES.
           05 FILLER PIC X(12) VALUE "SVC-ESCAPE".
           05 FILLER PIC X(8) VALUE "P0562".
           05 FILLER PIC X(60) VALUE "Tensão do sistema baixa".
           05 FILLER PIC X(12) VALUE "BATERIA".
           05 FILLER PIC X(12) VALUE "ALTERNADOR".
           05 FILLER PIC X(12) VALUE SPACES.
           05 FILLER PIC X(12) VALUE "SVC-BATERIA".
           05 FILLER PIC X(8) VALUE "P0217".
           05 FILLER PIC X(60) VALUE "Superaquecimento do motor".
           05 FILLER PIC X(12) VALUE "TERMOSTATO".
           05 FILLER PIC X(12) VALUE "BOMBA-AGUA".
           05 FILLER PIC X(12) VALUE SPACES.
           05 FILLER PIC X(12) VALUE "SVC-ARREFEC".
           05 FILLER PIC X(8) VALUE "P0335".
           05 FILLER PIC X(60) VALUE "Sensor de rotação do virabrequim".
           05 FILLER PIC X(12) VALUE "SENSOR-ROT".
           05 FILLER PIC X(12) VALUE SPACES.
           05 FILLER PIC X(12) VALUE SPACES.
           05 FILLER PIC X(12) VALUE "SVC-IGNICAO".
           05 FILLER PIC X(8) VALUE "P0401".
           05 FILLER PIC X(60) VALUE "Fluxo EGR insuficiente".
           05 FILLER PIC X(12) VALUE "VALVULA-EGR".
           05 FILLER PIC X(12) VALUE SPACES.
           05 FILLER PIC X(12) VALUE SPACES.
           05 FILLER PIC X(12) VALUE "SVC-EGR".
           05 FILLER PIC X(8) VALUE "P0087".
           05 FILLER PIC X(60) VALUE "Pressão do combustível baixa".
           05 FILLER PIC X(12) VALUE "FILTRO-COMB".
           05 FILLER PIC X(12) VALUE "INJETOR".
           05 FILLER PIC X(12) VALUE SPACES.
           05 FILLER PIC X(12) VALUE "SVC-INJECAO".
           05 FILLER PIC X(8) VALUE "C0561".
           05 FILLER PIC X(60) VALUE "Desgaste das pastilhas de freio".
           05 FILLER PIC X(12) VALUE "PAD-FREIO".
           05 FILLER PIC X(12) VALUE "DISCO-FREIO".
           05 FILLER PIC X(12) VALUE SPACES.
           05 FILLER PIC X(12) VALUE "SVC-FREIO".
       01 FLT-TABLE REDEFINES FLT-DATA.
           05 FLT-ROW OCCURS 10 TIMES.
               10 FR-CODE                        PIC X(8).
               10 FR-DESC                        PIC X(60).
               10 FR-PART-1                      PIC X(12).
               10 FR-PART-2                      PIC X(12).
               10 FR-PART-3                      PIC X(12).
               10 FR-SERVICE                     PIC X(12).

      *> Number sequences, all starting at zero.
       01 SEQ-DATA.
           05 FILLER PIC X(12) VALUE "CUSTOMER".
           05 FILLER PIC X(12) VALUE "DIAGNOSIS".
           05 FILLER PIC X(12) VALUE "QUOTE".
           05 FILLER PIC X(12) VALUE "PAYMENT".
           05 FILLER PIC X(12) VALUE "QUEUE".
           05 FILLER PIC X(12) VALUE "REFUND".
           05 FILLER PIC X(12) VALUE "NOTIFY".
           05 FILLER PIC X(12) VALUE "AUDIT".
           05 FILLER PIC X(12) VALUE "TOKEN".
       01 SEQ-TABLE REDEFINES SEQ-DATA.
           05 SEQ-ROW OCCURS 9 TIMES.
               10 SQ-NAME                        PIC X(12).

       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM LOAD-CATALOG
           PERFORM LOAD-FAULT-RULES
           PERFORM LOAD-SEQUENCES
           DISPLAY "SEED-LOAD  PowerOficina starting data"
           DISPLAY "  catalog       new " WS-CAT-NEW " skipped " WS-CAT-SKIP
           DISPLAY "  fault rules   new " WS-FLT-NEW " skipped " WS-FLT-SKIP
           DISPLAY "  sequences     new " WS-SEQ-NEW " skipped " WS-SEQ-SKIP
           DISPLAY "  failures      " WS-FAIL
           STOP RUN.

       LOAD-CATALOG.
           OPEN    I-O CATALOG-FILE
           IF    WS-FS-CAT = "35"
               OPEN    OUTPUT CATALOG-FILE
               CLOSE   CATALOG-FILE
               OPEN    I-O CATALOG-FILE
           END-IF
           IF    WS-FS-CAT NOT = "00"
               ADD     1                         TO    WS-FAIL
           ELSE
               PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 36
                   PERFORM VARYING WS-CUR FROM 1 BY 1 UNTIL WS-CUR > 2
                       PERFORM WRITE-CATALOG-ROW
                   END-PERFORM
               END-PERFORM
               CLOSE   CATALOG-FILE
           END-IF.

       WRITE-CATALOG-ROW.
           INITIALIZE CATALOG-RECORD
           MOVE    CR-CODE (WS-I)                TO    CAT-CODE
           MOVE    CR-KIND (WS-I)                TO    CAT-KIND
           MOVE    CR-NAME (WS-I)                TO    CAT-NAME
           MOVE    CR-HOURS (WS-I)               TO    CAT-HOURS
           IF    WS-CUR = 1
               MOVE    "BRL"                     TO    CAT-CURRENCY
               MOVE    CR-BRL (WS-I)             TO    CAT-PRICE
               IF    CR-KIND (WS-I) = "P"
                   MOVE    18.00                 TO    CAT-TAX-RATE
               ELSE
                   MOVE    5.00                  TO    CAT-TAX-RATE
               END-IF
           ELSE
               MOVE    "EUR"                     TO    CAT-CURRENCY
               MOVE    CR-EUR (WS-I)             TO    CAT-PRICE
               MOVE    23.00                     TO    CAT-TAX-RATE
           END-IF
           PERFORM INSERT-CATALOG-ROW
           IF    WS-PQ-STATUS = "00"
               ADD     1                         TO    WS-CAT-NEW
           ELSE
               ADD     1                         TO    WS-CAT-SKIP
           END-IF.

       LOAD-FAULT-RULES.
           OPEN    I-O FAULT-RULE-FILE
           IF    WS-FS-FLT = "35"
               OPEN    OUTPUT FAULT-RULE-FILE
               CLOSE   FAULT-RULE-FILE
               OPEN    I-O FAULT-RULE-FILE
           END-IF
           IF    WS-FS-FLT NOT = "00"
               ADD     1                         TO    WS-FAIL
           ELSE
               PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 10
                   INITIALIZE FAULT-RULE-RECORD
                   MOVE    FR-CODE (WS-I)        TO    FLT-CODE
                   MOVE    FR-DESC (WS-I)        TO    FLT-DESC
                   MOVE    FR-PART-1 (WS-I)      TO    FLT-PART-1
                   MOVE    FR-PART-2 (WS-I)      TO    FLT-PART-2
                   MOVE    FR-PART-3 (WS-I)      TO    FLT-PART-3
                   MOVE    FR-SERVICE (WS-I)     TO    FLT-SERVICE
                   PERFORM INSERT-FAULT-RULE-ROW
                   IF    WS-PQ-STATUS = "00"
                       ADD     1                 TO    WS-FLT-NEW
                   ELSE
                       ADD     1                 TO    WS-FLT-SKIP
                   END-IF
               END-PERFORM
               CLOSE   FAULT-RULE-FILE
           END-IF.

       LOAD-SEQUENCES.
           OPEN    I-O SEQUENCE-FILE
           IF    WS-FS-SEQ = "35"
               OPEN    OUTPUT SEQUENCE-FILE
               CLOSE   SEQUENCE-FILE
               OPEN    I-O SEQUENCE-FILE
           END-IF
           IF    WS-FS-SEQ NOT = "00"
               ADD     1                         TO    WS-FAIL
           ELSE
               PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 9
                   INITIALIZE SEQUENCE-RECORD
                   MOVE    SQ-NAME (WS-I)        TO    SEQ-NAME
                   MOVE    0                     TO    SEQ-VALUE
                   PERFORM INSERT-SEQUENCE-ROW
                   IF    WS-PQ-STATUS = "00"
                       ADD     1                 TO    WS-SEQ-NEW
                   ELSE
                       ADD     1                 TO    WS-SEQ-SKIP
                   END-IF
               END-PERFORM
               CLOSE   SEQUENCE-FILE
           END-IF.

           COPY "backend/COPYBOOKS/pq-catalog".
           COPY "backend/COPYBOOKS/pq-faultrules".
           COPY "backend/COPYBOOKS/pq-sequences".
