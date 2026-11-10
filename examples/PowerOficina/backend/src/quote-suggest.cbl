       IDENTIFICATION DIVISION.
       PROGRAM-ID. QUOTE-SUGGEST.
      *> Suggests the parts and services a diagnosis calls for (R9): for each fault code of the diagnosis, the fault
      *> rule names up to three parts and one service; each is looked up in the catalog of the asked currency and a
      *> code that two faults share is listed once. A service line shows its standard hours (R11). Nothing is saved:
      *> the consultant adds, removes or changes lines on the screen and QUOTE-SAVE makes the quote (R10).
      *> A part or service the catalog does not hold in that currency is left out, never priced in another currency.
      *> The request and the answer are in backend/COPYBOOKS/req-quote-suggest.cpy and res-quote-suggest.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT DIAGNOSIS-FILE ASSIGN TO "data/diagnoses.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS DGN-ID
               ALTERNATE RECORD KEY IS DGN-PLATE WITH DUPLICATES
               FILE STATUS IS WS-FS-DGN.
           SELECT FAULT-RULE-FILE ASSIGN TO "data/faultrules.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS FLT-CODE
               FILE STATUS IS WS-FS-FLT.
           SELECT CATALOG-FILE ASSIGN TO "data/catalog.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CAT-KEY
               FILE STATUS IS WS-FS-CAT.

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/diagnoses.FD".
           COPY "COPYBOOKS/faultrules.FD".
           COPY "COPYBOOKS/catalog.FD".

       WORKING-STORAGE SECTION.
       01 WS-FS-DGN                              PIC    XX.
       01 WS-FS-FLT                              PIC    XX.
       01 WS-FS-CAT                              PIC    XX.
       01 WS-F                                   PIC    9.
       01 WS-K                                   PIC    9.
       01 WS-J                                   PIC    999.
       01 WS-FAULT                               PIC    X(8).
       01 WS-WANT-CODE                           PIC    X(12).
       01 WS-DUP                                 PIC    9.
           COPY "backend/COPYBOOKS/ws-partiql".

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-quote-suggest".
           COPY "backend/COPYBOOKS/res-quote-suggest".

       PROCEDURE DIVISION USING QSG-REQ RETURNING QSG-RES.
       MAIN-PARA.
           INITIALIZE QSG-RES
           MOVE    "OK"                          TO    QSS-STATUS

           IF    QSQ-CURRENCY NOT = "BRL" AND QSQ-CURRENCY NOT = "EUR"
               MOVE    "INVALID"                 TO    QSS-STATUS
               MOVE    "A moeda deve ser BRL ou EUR."
                                                 TO    QSS-MESSAGE
           END-IF

           IF    QSS-STATUS = "OK"
               MOVE    "NODIAG"                  TO    QSS-STATUS
               MOVE    "Diagnóstico não encontrado."
                                                 TO    QSS-MESSAGE
               OPEN    INPUT DIAGNOSIS-FILE
               IF    WS-FS-DGN = "00"
                   MOVE    QSQ-DGN-ID            TO    DGN-ID
                   PERFORM GET-DIAGNOSIS-ROW
                   IF    WS-PQ-STATUS = "00"
                       MOVE    "OK"              TO    QSS-STATUS
                       MOVE    SPACES            TO    QSS-MESSAGE
                   END-IF
                   CLOSE   DIAGNOSIS-FILE
               END-IF
           END-IF

           IF    QSS-STATUS = "OK"
               OPEN    INPUT FAULT-RULE-FILE
               OPEN    INPUT CATALOG-FILE
               IF    WS-FS-FLT = "00" AND WS-FS-CAT = "00"
                   PERFORM VARYING WS-F FROM 1 BY 1 UNTIL WS-F > 5
                       PERFORM PICK-FAULT
                       IF    WS-FAULT NOT = SPACES
                           PERFORM ADD-RULE-ITEMS
                       END-IF
                   END-PERFORM
                   CLOSE   FAULT-RULE-FILE
                   CLOSE   CATALOG-FILE
               END-IF
               IF    QSS-COUNT = 0
                   MOVE    "Nenhum código de falha pede peça ou serviço."
                                                 TO    QSS-MESSAGE
               END-IF
           END-IF

           GOBACK.

       PICK-FAULT.
           EVALUATE WS-F
               WHEN 1  MOVE    DGN-FAULT-1       TO    WS-FAULT
               WHEN 2  MOVE    DGN-FAULT-2       TO    WS-FAULT
               WHEN 3  MOVE    DGN-FAULT-3       TO    WS-FAULT
               WHEN 4  MOVE    DGN-FAULT-4       TO    WS-FAULT
               WHEN OTHER
                   MOVE    DGN-FAULT-5           TO    WS-FAULT
           END-EVALUATE.

       ADD-RULE-ITEMS.
           MOVE    WS-FAULT                      TO    FLT-CODE
           PERFORM GET-FAULT-RULE-ROW
           IF    WS-PQ-STATUS = "00"
               MOVE    FLT-PART-1                TO    WS-WANT-CODE
               PERFORM ADD-ONE-ITEM
               MOVE    FLT-PART-2                TO    WS-WANT-CODE
               PERFORM ADD-ONE-ITEM
               MOVE    FLT-PART-3                TO    WS-WANT-CODE
               PERFORM ADD-ONE-ITEM
               MOVE    FLT-SERVICE               TO    WS-WANT-CODE
               PERFORM ADD-ONE-ITEM
           END-IF.

      *> ADD-ONE-ITEM: the catalog row WS-WANT-CODE of the currency goes on the list, once, if there is room.
       ADD-ONE-ITEM.
           IF    WS-WANT-CODE NOT = SPACES AND QSS-COUNT < 20
               MOVE    0                         TO    WS-DUP
               PERFORM VARYING WS-J FROM 1 BY 1 UNTIL WS-J > QSS-COUNT
                   IF    QSL-CODE (WS-J) = WS-WANT-CODE
                       MOVE    1                 TO    WS-DUP
                   END-IF
               END-PERFORM

               IF    WS-DUP = 0
                   MOVE    QSQ-CURRENCY          TO    CAT-CURRENCY
                   MOVE    WS-WANT-CODE          TO    CAT-CODE
                   PERFORM GET-CATALOG-ROW
                   IF    WS-PQ-STATUS = "00"
                       ADD     1                 TO    QSS-COUNT
                       MOVE    CAT-KIND          TO    QSL-KIND (QSS-COUNT)
                       MOVE    CAT-CODE          TO    QSL-CODE (QSS-COUNT)
                       MOVE    CAT-NAME          TO    QSL-NAME (QSS-COUNT)
                       MOVE    CAT-PRICE         TO    QSL-UNIT (QSS-COUNT)
                       MOVE    CAT-HOURS         TO    QSL-HOURS (QSS-COUNT)
                       MOVE    WS-FAULT          TO    QSL-FAULT (QSS-COUNT)
                       MOVE    1                 TO    QSL-QTY (QSS-COUNT)
                       IF    CAT-KIND = "S"
                           MOVE    CAT-HOURS     TO    QSL-QTY (QSS-COUNT)
                       END-IF
                   END-IF
               END-IF
           END-IF.

           COPY "backend/COPYBOOKS/pq-diagnoses".
           COPY "backend/COPYBOOKS/pq-faultrules".
           COPY "backend/COPYBOOKS/pq-catalog".
