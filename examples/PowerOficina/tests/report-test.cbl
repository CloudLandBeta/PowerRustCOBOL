       IDENTIFICATION DIVISION.
       PROGRAM-ID. REPORT-TEST.
      *> Smoke test of REPORT-LIST on an empty system: every report answers OK with no rows and a zero total; a wrong
      *> mode and a wrong currency are refused. Run in an empty folder with a data/ folder (assembly step adds the program).

       DATA DIVISION.
       WORKING-STORAGE SECTION.
           COPY "backend/COPYBOOKS/req-report".
           COPY "backend/COPYBOOKS/res-report".
       01 WS-RUN                                 PIC    9(3)   VALUE 0.
       01 WS-PASS                                PIC    9(3)   VALUE 0.
       01 WS-FAIL                                PIC    9(3)   VALUE 0.
       01 WS-OK                                  PIC    9.
       01 WS-NAME                                PIC    X(50).

       PROCEDURE DIVISION.
       MAIN-PARA.
           MOVE    "Q" TO RPQ-MODE
           MOVE    "BRL" TO RPQ-CURRENCY
           MOVE    SPACES TO RPQ-FILTER
           CALL "REPORT-LIST" USING RPT-REQ RETURNING RPT-RES
           MOVE    "quotes, empty system" TO WS-NAME
           MOVE    0 TO WS-OK
           IF RPS-STATUS = "OK" AND RPS-COUNT = 0 AND RPS-TOTAL = 0 MOVE 1 TO WS-OK END-IF
           PERFORM TALLY

           MOVE    "P" TO RPQ-MODE
           CALL "REPORT-LIST" USING RPT-REQ RETURNING RPT-RES
           MOVE    "payments, empty system" TO WS-NAME
           MOVE    0 TO WS-OK
           IF RPS-STATUS = "OK" AND RPS-COUNT = 0 AND RPS-TOTAL = 0 MOVE 1 TO WS-OK END-IF
           PERFORM TALLY

           MOVE    "F" TO RPQ-MODE
           MOVE    "EUR" TO RPQ-CURRENCY
           CALL "REPORT-LIST" USING RPT-REQ RETURNING RPT-RES
           MOVE    "failed installments, empty system" TO WS-NAME
           MOVE    0 TO WS-OK
           IF RPS-STATUS = "OK" AND RPS-COUNT = 0 MOVE 1 TO WS-OK END-IF
           PERFORM TALLY

           MOVE    "N" TO RPQ-MODE
           CALL "REPORT-LIST" USING RPT-REQ RETURNING RPT-RES
           MOVE    "notices, empty system" TO WS-NAME
           MOVE    0 TO WS-OK
           IF RPS-STATUS = "OK" AND RPS-COUNT = 0 MOVE 1 TO WS-OK END-IF
           PERFORM TALLY

           MOVE    "Q" TO RPQ-MODE
           MOVE    "USD" TO RPQ-CURRENCY
           CALL "REPORT-LIST" USING RPT-REQ RETURNING RPT-RES
           MOVE    "wrong currency refused" TO WS-NAME
           MOVE    0 TO WS-OK
           IF RPS-STATUS = "INVALID" MOVE 1 TO WS-OK END-IF
           PERFORM TALLY

           MOVE    "X" TO RPQ-MODE
           CALL "REPORT-LIST" USING RPT-REQ RETURNING RPT-RES
           MOVE    "wrong mode refused" TO WS-NAME
           MOVE    0 TO WS-OK
           IF RPS-STATUS = "INVALID" MOVE 1 TO WS-OK END-IF
           PERFORM TALLY

           DISPLAY "Report test (REPORT-LIST, empty system)"
           DISPLAY "  cases run        " WS-RUN
           DISPLAY "  passed           " WS-PASS
           DISPLAY "  failed           " WS-FAIL
           IF WS-FAIL = 0 DISPLAY "  result           PASS" ELSE DISPLAY "  result           FAIL" END-IF
           STOP RUN.

       TALLY.
           ADD 1 TO WS-RUN
           IF WS-OK = 1 ADD 1 TO WS-PASS ELSE ADD 1 TO WS-FAIL DISPLAY "  FAIL: " WS-NAME END-IF.
