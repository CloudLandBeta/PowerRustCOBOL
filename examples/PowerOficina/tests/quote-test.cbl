       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUOTE-TEST.
      *> Test of the Backend programs CATALOG-LIST, QUOTE-SUGGEST, QUOTE-SAVE and QUOTE-GET (CUSTOMER-SAVE,
      *> VEHICLE-SAVE and DIAG-COLLECT give them a customer, a vehicle and a diagnosis).
      *> A driver: the programs under test are added to this file as contained programs by the assembly step.
      *> Run it in an empty folder that has a data/ folder holding copies of the project's catalog.idx, faultrules.idx
      *> and sequences.idx.
      *> Cases: the catalog by currency and kind; suggestions from fault codes (shared parts once, no fault, unknown
      *> diagnosis); a BRL quote and an EUR quote with the same lines, each with its own prices, taxes per line, the
      *> total without taxes and the total; service hours adjusted with and without a reason; the execution date
      *> suggested and an agreed one, a date in the past; a revision that leaves the first one as it was; a change of
      *> currency that makes a new quote and leaves the original; every invalid request.

       DATA DIVISION.
       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/req-customer".
           COPY "backend/COPYBOOKS/res-customer".
           COPY "backend/COPYBOOKS/req-vehicle".
           COPY "backend/COPYBOOKS/res-vehicle".
           COPY "backend/COPYBOOKS/req-diag".
           COPY "backend/COPYBOOKS/res-diag".
           COPY "backend/COPYBOOKS/req-catalog-list".
           COPY "backend/COPYBOOKS/res-catalog-list".
           COPY "backend/COPYBOOKS/req-quote-suggest".
           COPY "backend/COPYBOOKS/res-quote-suggest".
           COPY "backend/COPYBOOKS/req-quote".
           COPY "backend/COPYBOOKS/res-quote".
           COPY "backend/COPYBOOKS/req-quote-get".
       01 WS-CASE-NAME                           PIC    X(60).
       01 WS-OK                                  PIC    9.
       01 WS-OWNER-ID                            PIC    9(8).
       01 WS-DGN-A                               PIC    9(8).
       01 WS-DGN-B                               PIC    9(8).
       01 WS-DGN-NONE                            PIC    9(8).
       01 WS-ID-BRL                              PIC    9(8).
       01 WS-ID-EUR                              PIC    9(8).
       01 WS-TODAY.
          05 WS-TODAY-DATE                       PIC    9(8).
          05 WS-TODAY-REST                       PIC    X(13).
       01 WS-DAYNUM                              PIC    9(9).
       01 WS-TOMORROW                            PIC    9(8).
       01 WS-IN-TEN                              PIC    9(8).
       01 WS-PASS                                PIC    999    VALUE 0.
       01 WS-FAIL                                PIC    999    VALUE 0.
       01 WS-CASES                               PIC    999    VALUE 0.
       01 WS-T0                                  PIC    9(8).
       01 WS-T1                                  PIC    9(8).
       01 WS-ELAPSED                             PIC    9(8).

       PROCEDURE DIVISION.
       MAIN-PARA.
           ACCEPT  WS-T0                         FROM  TIME
           MOVE    FUNCTION CURRENT-DATE         TO    WS-TODAY
           COMPUTE WS-DAYNUM = FUNCTION INTEGER-OF-DATE (WS-TODAY-DATE) + 1
           COMPUTE WS-TOMORROW = FUNCTION DATE-OF-INTEGER (WS-DAYNUM)
           COMPUTE WS-DAYNUM = FUNCTION INTEGER-OF-DATE (WS-TODAY-DATE) + 10
           COMPUTE WS-IN-TEN = FUNCTION DATE-OF-INTEGER (WS-DAYNUM)

           PERFORM MAKE-BASE-DATA

           PERFORM TEST-CATALOG
           PERFORM TEST-SUGGEST
           PERFORM TEST-QUOTES
           PERFORM TEST-INVALID

           ACCEPT  WS-T1                         FROM  TIME
           COMPUTE WS-ELAPSED = WS-T1 - WS-T0

           DISPLAY "Quote programs test (CATALOG-LIST, QUOTE-SUGGEST, QUOTE-SAVE, QUOTE-GET)"
           DISPLAY "  cases run      " WS-CASES
           DISPLAY "  passed         " WS-PASS
           DISPLAY "  failed         " WS-FAIL
           DISPLAY "  elapsed        " WS-ELAPSED " hundredths of a second"
           IF    WS-FAIL = 0
               DISPLAY "  result         PASS"
           ELSE
               DISPLAY "  result         FAIL"
           END-IF
           STOP RUN.

       MAKE-BASE-DATA.
           INITIALIZE CUS-REQ
           MOVE    "João da Silva"               TO    CRQ-NAME
           MOVE    "52998224725"                 TO    CRQ-DOC
           MOVE    "N"                           TO    CRQ-CONSENT-EMAIL
           MOVE    "N"                           TO    CRQ-CONSENT-WA
           MOVE    "Maria Atendente"             TO    CRQ-OPERATOR
           CALL    "CUSTOMER-SAVE" USING CUS-REQ RETURNING CUS-RES
           MOVE    CRS-ID                        TO    WS-OWNER-ID

           PERFORM MAKE-VEHICLE-ABC
           PERFORM MAKE-VEHICLE-XYZ

      *> diagnosis A of ABC1D23: P0300 and P0301 share the spark plugs and the coil; B of XYZ4E56 has no fault code
           INITIALIZE DGC-REQ
           MOVE    "ABC1D23"                     TO    DCQ-PLATE
           MOVE    "Maria Atendente"             TO    DCQ-OPERATOR
           MOVE    "M"                           TO    DCQ-MODE
           MOVE    90                            TO    DCQ-ENGINE-TEMP
           MOVE    800                           TO    DCQ-RPM
           MOVE    12.6                          TO    DCQ-BATTERY
           MOVE    300.0                         TO    DCQ-OIL-PRESSURE
           MOVE    "P0300"                       TO    DCQ-FAULT-1
           MOVE    "P0301"                       TO    DCQ-FAULT-2
           CALL    "DIAG-COLLECT" USING DGC-REQ RETURNING DGC-RES
           MOVE    DCS-ID                        TO    WS-DGN-A

           INITIALIZE DGC-REQ
           MOVE    "XYZ4E56"                     TO    DCQ-PLATE
           MOVE    "Maria Atendente"             TO    DCQ-OPERATOR
           MOVE    "M"                           TO    DCQ-MODE
           MOVE    90                            TO    DCQ-ENGINE-TEMP
           MOVE    800                           TO    DCQ-RPM
           MOVE    12.6                          TO    DCQ-BATTERY
           MOVE    300.0                         TO    DCQ-OIL-PRESSURE
           CALL    "DIAG-COLLECT" USING DGC-REQ RETURNING DGC-RES
           MOVE    DCS-ID                        TO    WS-DGN-B.

       MAKE-VEHICLE-ABC.
           INITIALIZE VEH-REQ
           MOVE    "ABC1D23"                     TO    VRQ-PLATE
           MOVE    WS-OWNER-ID                   TO    VRQ-CUS-ID
           MOVE    "Fiat"                        TO    VRQ-BRAND
           MOVE    "Palio 1.0"                   TO    VRQ-MODEL
           MOVE    2014                          TO    VRQ-YEAR
           MOVE    "Maria Atendente"             TO    VRQ-OPERATOR
           CALL    "VEHICLE-SAVE" USING VEH-REQ RETURNING VEH-RES.

       MAKE-VEHICLE-XYZ.
           INITIALIZE VEH-REQ
           MOVE    "XYZ4E56"                     TO    VRQ-PLATE
           MOVE    WS-OWNER-ID                   TO    VRQ-CUS-ID
           MOVE    "VW"                          TO    VRQ-BRAND
           MOVE    "Gol 1.6"                     TO    VRQ-MODEL
           MOVE    2018                          TO    VRQ-YEAR
           MOVE    "Maria Atendente"             TO    VRQ-OPERATOR
           CALL    "VEHICLE-SAVE" USING VEH-REQ RETURNING VEH-RES.

       TALLY.
           ADD     1                             TO    WS-CASES
           IF    WS-OK = 1
               ADD     1                         TO    WS-PASS
           ELSE
               ADD     1                         TO    WS-FAIL
               DISPLAY "  FAILED: " WS-CASE-NAME
           END-IF.

       TEST-CATALOG.
           MOVE    "catalog: BRL add-ons are 4, each with price and hours"  TO WS-CASE-NAME
           MOVE    "BRL"                         TO    CLQ-CURRENCY
           MOVE    "A"                           TO    CLQ-KIND
           CALL    "CATALOG-LIST" USING CLS-REQ RETURNING CLS-RES
           MOVE    0                             TO    WS-OK
           IF    CLR-STATUS = "OK" AND CLR-COUNT = 4 AND CLR-UNIT (1) > 0 AND CLR-HOURS (1) > 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "catalog: EUR list has its own prices (wash 19.00)"  TO WS-CASE-NAME
           MOVE    "EUR"                         TO    CLQ-CURRENCY
           CALL    "CATALOG-LIST" USING CLS-REQ RETURNING CLS-RES
           MOVE    0                             TO    WS-OK
           IF    CLR-STATUS = "OK" AND CLR-COUNT = 4
                 AND CLR-CODE (4) = "ADD-OLEO" AND CLR-UNIT (3) = 19.00
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "catalog: parts of BRL are many, none of kind A"  TO WS-CASE-NAME
           MOVE    "BRL"                         TO    CLQ-CURRENCY
           MOVE    "P"                           TO    CLQ-KIND
           CALL    "CATALOG-LIST" USING CLS-REQ RETURNING CLS-RES
           MOVE    0                             TO    WS-OK
           IF    CLR-STATUS = "OK" AND CLR-COUNT > 10 AND CLR-COUNT < 31
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "catalog: currency USD is refused"  TO WS-CASE-NAME
           MOVE    "USD"                         TO    CLQ-CURRENCY
           CALL    "CATALOG-LIST" USING CLS-REQ RETURNING CLS-RES
           MOVE    0                             TO    WS-OK
           IF    CLR-STATUS = "INVALID" AND CLR-COUNT = 0
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

       TEST-SUGGEST.
           MOVE    "suggest: P0300 + P0301 give 4 lines, shared parts once"  TO WS-CASE-NAME
           INITIALIZE QSG-REQ
           MOVE    WS-DGN-A                      TO    QSQ-DGN-ID
           MOVE    "BRL"                         TO    QSQ-CURRENCY
           CALL    "QUOTE-SUGGEST" USING QSG-REQ RETURNING QSG-RES
           MOVE    0                             TO    WS-OK
           IF    QSS-STATUS = "OK" AND QSS-COUNT = 4
                 AND QSL-CODE (1) = "VELA" AND QSL-CODE (2) = "BOBINA"
                 AND QSL-CODE (3) = "FILTRO-COMB" AND QSL-CODE (4) = "SVC-IGNICAO"
                 AND QSL-UNIT (1) = 160.00 AND QSL-KIND (4) = "S" AND QSL-QTY (4) = 1.00
                 AND QSL-FAULT (1) = "P0300"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "suggest: same diagnosis in EUR, EUR price of the spark plugs"  TO WS-CASE-NAME
           MOVE    "EUR"                         TO    QSQ-CURRENCY
           CALL    "QUOTE-SUGGEST" USING QSG-REQ RETURNING QSG-RES
           MOVE    0                             TO    WS-OK
           IF    QSS-STATUS = "OK" AND QSS-COUNT = 4 AND QSL-UNIT (1) = 38.00
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "suggest: a diagnosis without fault codes lists nothing"  TO WS-CASE-NAME
           MOVE    WS-DGN-B                      TO    QSQ-DGN-ID
           CALL    "QUOTE-SUGGEST" USING QSG-REQ RETURNING QSG-RES
           MOVE    0                             TO    WS-OK
           IF    QSS-STATUS = "OK" AND QSS-COUNT = 0 AND QSS-MESSAGE NOT = SPACES
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "suggest: unknown diagnosis"  TO    WS-CASE-NAME
           MOVE    99999                         TO    QSQ-DGN-ID
           CALL    "QUOTE-SUGGEST" USING QSG-REQ RETURNING QSG-RES
           MOVE    0                             TO    WS-OK
           IF    QSS-STATUS = "NODIAG"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "suggest: currency USD is refused"  TO WS-CASE-NAME
           MOVE    WS-DGN-A                      TO    QSQ-DGN-ID
           MOVE    "USD"                         TO    QSQ-CURRENCY
           CALL    "QUOTE-SUGGEST" USING QSG-REQ RETURNING QSG-RES
           MOVE    0                             TO    WS-OK
           IF    QSS-STATUS = "INVALID"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

      *> BASE-QUOTE: spark plugs x2 (part), the ignition service (standard 1 hour) and the wash (add-on).
       BASE-QUOTE.
           INITIALIZE QUO-REQ
           MOVE    "ABC1D23"                     TO    QRQ-PLATE
           MOVE    WS-DGN-A                      TO    QRQ-DGN-ID
           MOVE    "BRL"                         TO    QRQ-CURRENCY
           MOVE    "Maria Atendente"             TO    QRQ-OPERATOR
           MOVE    3                             TO    QRQ-COUNT
           MOVE    "P"                           TO    QRL-KIND (1)
           MOVE    "VELA"                        TO    QRL-CODE (1)
           MOVE    2                             TO    QRL-QTY (1)
           MOVE    "S"                           TO    QRL-KIND (2)
           MOVE    "SVC-IGNICAO"                 TO    QRL-CODE (2)
           MOVE    "A"                           TO    QRL-KIND (3)
           MOVE    "ADD-LAVAGEM"                 TO    QRL-CODE (3).

       TEST-QUOTES.
           MOVE    "BRL quote: amounts, taxes per line, subtotal and total"  TO WS-CASE-NAME
           PERFORM BASE-QUOTE
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES
           MOVE    QTS-ID                        TO    WS-ID-BRL
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "OK" AND QTS-ACTION = "CREATED" AND QTS-REV = 1 AND QTS-ID > 0
                 AND QTS-COUNT = 3
                 AND QTL-AMOUNT (1) = 320.00 AND QTL-TAX (1) = 57.60
                 AND QTL-AMOUNT (2) = 150.00 AND QTL-TAX (2) = 7.50 AND QTL-QTY (2) = 1.00
                 AND QTL-AMOUNT (3) = 80.00 AND QTL-TAX (3) = 4.00
                 AND QTS-SUBTOTAL = 550.00 AND QTS-TAX = 69.10 AND QTS-TOTAL = 619.10
                 AND QTS-HOURS = 2.00 AND QTS-QSTATUS = "ABERTO" AND QTS-CURRENCY = "BRL"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "BRL quote: execution date suggested is tomorrow, valid 7 days"  TO WS-CASE-NAME
           MOVE    0                             TO    WS-OK
           IF    QTS-SUGGESTED = WS-TOMORROW AND QTS-DEADLINE = WS-TOMORROW AND QTS-VALID-UNTIL > WS-TODAY-DATE
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "BRL quote: customer comes from the vehicle's owner"  TO WS-CASE-NAME
           MOVE    0                             TO    WS-OK
           IF    QTS-CUS-ID = WS-OWNER-ID AND QTS-PLATE = "ABC1D23" AND QTS-DGN-ID = WS-DGN-A
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "EUR quote: same lines, own prices and own tax rates"  TO WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    "EUR"                         TO    QRQ-CURRENCY
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES
           MOVE    QTS-ID                        TO    WS-ID-EUR
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "OK" AND QTS-ACTION = "CREATED" AND QTS-ID NOT = WS-ID-BRL
                 AND QTL-AMOUNT (1) = 76.00 AND QTL-TAX (1) = 17.48
                 AND QTL-AMOUNT (2) = 45.00 AND QTL-TAX (2) = 10.35
                 AND QTL-AMOUNT (3) = 19.00 AND QTL-TAX (3) = 4.37
                 AND QTS-SUBTOTAL = 140.00 AND QTS-TAX = 32.20 AND QTS-TOTAL = 172.20
                 AND QTS-CURRENCY = "EUR"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "service hours changed with a reason: 2.5 h, marked manual"  TO WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    2.5                           TO    QRL-HOURS (2)
           MOVE    "Cabeamento danificado, mais tempo."  TO QRL-NOTE (2)
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "OK" AND QTL-QTY (2) = 2.50 AND QTL-AMOUNT (2) = 375.00
                 AND QTL-TAX (2) = 18.75 AND QTL-MANUAL (2) = "S" AND QTS-HOURS = 3.50
                 AND QTS-SUBTOTAL = 775.00
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "service hours changed without a reason are refused"  TO WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    2.5                           TO    QRL-HOURS (2)
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "INVALID" AND QTS-FIELD = "NOTE" AND QTS-LINE-NO = 2
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "agreed execution date is kept when it is not in the past"  TO WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    WS-IN-TEN                     TO    QRQ-DEADLINE
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "OK" AND QTS-DEADLINE = WS-IN-TEN AND QTS-SUGGESTED = WS-TOMORROW
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "a revision of the BRL quote: same number, revision 2"  TO WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    WS-ID-BRL                     TO    QRQ-ID
           MOVE    4                             TO    QRL-QTY (1)
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "OK" AND QTS-ACTION = "REVISED" AND QTS-ID = WS-ID-BRL AND QTS-REV = 2
                 AND QTL-AMOUNT (1) = 640.00 AND QTS-SUBTOTAL = 870.00
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "get: revision 1 is as it was before the revision"  TO WS-CASE-NAME
           MOVE    WS-ID-BRL                     TO    QGQ-ID
           MOVE    1                             TO    QGQ-REV
           CALL    "QUOTE-GET" USING QGT-REQ RETURNING QUO-RES
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "OK" AND QTS-REV = 1 AND QTS-TOTAL = 619.10 AND QTL-AMOUNT (1) = 320.00
                 AND QTS-COUNT = 3
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "get: revision 0 answers the latest one"  TO WS-CASE-NAME
           MOVE    0                             TO    QGQ-REV
           CALL    "QUOTE-GET" USING QGT-REQ RETURNING QUO-RES
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "OK" AND QTS-REV = 2 AND QTL-AMOUNT (1) = 640.00
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "changing the currency makes a NEW quote in EUR"  TO WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    WS-ID-BRL                     TO    QRQ-ID
           MOVE    "EUR"                         TO    QRQ-CURRENCY
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "OK" AND QTS-ACTION = "NEWCURRENCY" AND QTS-ID NOT = WS-ID-BRL
                 AND QTS-REV = 1 AND QTS-CURRENCY = "EUR" AND QTS-SUBTOTAL = 140.00
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "get: the original BRL quote is untouched by that change"  TO WS-CASE-NAME
           MOVE    WS-ID-BRL                     TO    QGQ-ID
           MOVE    0                             TO    QGQ-REV
           CALL    "QUOTE-GET" USING QGT-REQ RETURNING QUO-RES
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "OK" AND QTS-REV = 2 AND QTS-CURRENCY = "BRL" AND QTS-SUBTOTAL = 870.00
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "get: a quote that does not exist"  TO WS-CASE-NAME
           MOVE    99999                         TO    QGQ-ID
           CALL    "QUOTE-GET" USING QGT-REQ RETURNING QUO-RES
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "NOTFOUND"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

       TEST-INVALID.
           MOVE    "operator missing"            TO    WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    SPACES                        TO    QRQ-OPERATOR
           PERFORM RUN-INVALID
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "INVALID" AND QTS-FIELD = "OPERATOR"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "plate too short"             TO    WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    "AB1"                         TO    QRQ-PLATE
           PERFORM RUN-INVALID
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "INVALID" AND QTS-FIELD = "PLATE"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "currency USD"                TO    WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    "USD"                         TO    QRQ-CURRENCY
           PERFORM RUN-INVALID
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "INVALID" AND QTS-FIELD = "CURRENCY"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "no lines"                    TO    WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    0                             TO    QRQ-COUNT
           PERFORM RUN-INVALID
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "INVALID" AND QTS-FIELD = "COUNT"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "vehicle not on file"         TO    WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    "QQQ9Q99"                     TO    QRQ-PLATE
           PERFORM RUN-INVALID
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "NOVEHICLE"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "diagnosis of another vehicle"  TO  WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    WS-DGN-B                      TO    QRQ-DGN-ID
           PERFORM RUN-INVALID
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "INVALID" AND QTS-FIELD = "DIAG"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "diagnosis that does not exist"  TO WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    99999                         TO    QRQ-DGN-ID
           PERFORM RUN-INVALID
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "NODIAG"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "code that is not in the catalog"  TO WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    "NAO-EXISTE"                  TO    QRL-CODE (2)
           PERFORM RUN-INVALID
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "INVALID" AND QTS-FIELD = "LINE" AND QTS-LINE-NO = 2
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "kind that does not match the catalog"  TO WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    "S"                           TO    QRL-KIND (1)
           PERFORM RUN-INVALID
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "INVALID" AND QTS-FIELD = "KIND" AND QTS-LINE-NO = 1
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "part with quantity zero"     TO    WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    0                             TO    QRL-QTY (1)
           PERFORM RUN-INVALID
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "INVALID" AND QTS-FIELD = "LINE" AND QTS-LINE-NO = 1
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "execution date in the past"  TO    WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    20200101                      TO    QRQ-DEADLINE
           PERFORM RUN-INVALID
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "INVALID" AND QTS-FIELD = "DEADLINE"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY

           MOVE    "revision of a quote that does not exist"  TO WS-CASE-NAME
           PERFORM BASE-QUOTE
           MOVE    99999                         TO    QRQ-ID
           PERFORM RUN-INVALID
           MOVE    0                             TO    WS-OK
           IF    QTS-STATUS = "NOTFOUND"
               MOVE    1                         TO    WS-OK
           END-IF
           PERFORM TALLY.

       RUN-INVALID.
           CALL    "QUOTE-SAVE" USING QUO-REQ RETURNING QUO-RES.
