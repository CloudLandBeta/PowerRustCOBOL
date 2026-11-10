       IDENTIFICATION DIVISION.
       PROGRAM-ID. CATALOG-LIST.
      *> Lists the catalog rows of one currency and one kind (P part, S service, A add-on), in code order, at most 30,
      *> so a screen can offer the add-ons (R13, R14, R15) or let the consultant add a part (R10). New add-ons added
      *> to the catalog appear here with no change of code (R15). A list is always of ONE currency (R42).
      *> The request and the answer are in backend/COPYBOOKS/req-catalog-list.cpy and res-catalog-list.cpy.

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT CATALOG-FILE ASSIGN TO "data/catalog.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CAT-KEY
               FILE STATUS IS WS-FS-CAT.

       DATA DIVISION.
       FILE SECTION.
           COPY "COPYBOOKS/catalog.FD".

       WORKING-STORAGE SECTION.
       01 WS-FS-CAT                              PIC    XX.
       01 WS-EOF                                 PIC    X.
           COPY "backend/COPYBOOKS/ws-partiql".

       LINKAGE SECTION.
           COPY "backend/COPYBOOKS/req-catalog-list".
           COPY "backend/COPYBOOKS/res-catalog-list".

       PROCEDURE DIVISION USING CLS-REQ RETURNING CLS-RES.
       MAIN-PARA.
           INITIALIZE CLS-RES
           MOVE    "OK"                          TO    CLR-STATUS

           IF    CLQ-CURRENCY NOT = "BRL" AND CLQ-CURRENCY NOT = "EUR"
               MOVE    "INVALID"                 TO    CLR-STATUS
               MOVE    "A moeda deve ser BRL ou EUR."
                                                 TO    CLR-MESSAGE
           END-IF

           IF    CLR-STATUS = "OK"
               OPEN    INPUT CATALOG-FILE
               IF    WS-FS-CAT = "00"
                   MOVE    CLQ-CURRENCY          TO    CAT-CURRENCY
                   MOVE    "N"                   TO    WS-EOF
                   PERFORM SELECT-CATALOG-OF-CURRENCY
                   IF    WS-PQ-STATUS NOT = "00"
                       MOVE    "Y"               TO    WS-EOF
                   END-IF

                   PERFORM UNTIL WS-EOF = "Y" OR CLR-COUNT = 30
                       PERFORM FETCH-CATALOG-ROW
                       IF    WS-PQ-STATUS NOT = "00"
                           MOVE    "Y"           TO    WS-EOF
                       ELSE
                           IF    CAT-CURRENCY NOT = CLQ-CURRENCY
                               MOVE    "Y"       TO    WS-EOF
                           ELSE
                               IF    CAT-KIND = CLQ-KIND
                                   ADD     1     TO    CLR-COUNT
                                   MOVE    CAT-CODE
                                                 TO    CLR-CODE (CLR-COUNT)
                                   MOVE    CAT-NAME
                                                 TO    CLR-NAME (CLR-COUNT)
                                   MOVE    CAT-PRICE
                                                 TO    CLR-UNIT (CLR-COUNT)
                                   MOVE    CAT-HOURS
                                                 TO    CLR-HOURS (CLR-COUNT)
                                   MOVE    CAT-TAX-RATE
                                                 TO    CLR-TAX-RATE (CLR-COUNT)
                               END-IF
                           END-IF
                       END-IF
                   END-PERFORM
                   CLOSE   CATALOG-FILE
               END-IF
           END-IF

           GOBACK.

           COPY "backend/COPYBOOKS/pq-catalog".
