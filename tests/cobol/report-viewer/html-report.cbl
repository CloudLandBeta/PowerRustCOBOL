       IDENTIFICATION DIVISION.
       PROGRAM-ID. RPTVWHT.

      *>---------------------------------------------------------------*
      *> RPTVWHT — spec 062, a report printed into a Viewer.
      *>
      *> ORGANIZATION IS HTML
      *>
      *> Exercises, and reports on:
      *>   - WRITE record — one record, one line
      *>   - the Viewer's HTML subset: headings, paragraphs, tables
      *>   - no CSS and no scripts, by design
      *>
      *> The result block is printed ONCE, at the end (GOLDEN RULE #7):
      *> no per-record DISPLAY, and every number below is this run's own.
      *>---------------------------------------------------------------*

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT REPORT-FILE ASSIGN TO VIEWER "VWR-1"
               ORGANIZATION IS HTML
               FILE STATUS IS WS-STATUS.

       DATA DIVISION.
       FILE SECTION.
       FD  REPORT-FILE.
       01  REPORT-LINE            PIC X(80).

       WORKING-STORAGE SECTION.
       01  WS-STATUS              PIC XX.
       01  WS-I                   PIC 9(6).
       01  WS-RECORDS             PIC 9(6) VALUE 0.
       01  WS-PAGES               PIC 9(6) VALUE 0.
       01  WS-PASS                PIC 9(4) VALUE 0.
       01  WS-FAIL                PIC 9(4) VALUE 0.
       01  WS-START               PIC 9(8) VALUE 0.
       01  WS-END                 PIC 9(8) VALUE 0.
       01  WS-MS                  PIC 9(8) VALUE 0.
       01  WS-RATE                PIC 9(9) VALUE 0.
       01  WS-NOW.
           05  WS-NOW-DATE        PIC X(8).
           05  WS-NOW-HH          PIC 99.
           05  WS-NOW-MM          PIC 99.
           05  WS-NOW-SS          PIC 99.
           05  WS-NOW-CS          PIC 99.
       01  WS-TEXT                PIC X(80).
       01  WS-SHOW-COUNT          PIC ZZZ,ZZ9.
       01  WS-SHOW-MS             PIC ZZZ,ZZ9.
       01  WS-SHOW-RATE           PIC ZZZ,ZZZ,ZZ9.

       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM CLOCK-START
           OPEN OUTPUT REPORT-FILE
           PERFORM CHECK-OPEN
           MOVE "<h1>Quarterly Sales</h1>" TO REPORT-LINE
           WRITE REPORT-LINE
           PERFORM CHECK-WRITE
           ADD 1 TO WS-RECORDS
           MOVE "<p>Written from COBOL. No CSS, no scripts.</p>"
               TO REPORT-LINE
           WRITE REPORT-LINE
           ADD 1 TO WS-RECORDS
           MOVE "<table>" TO REPORT-LINE
           WRITE REPORT-LINE
           ADD 1 TO WS-RECORDS
           MOVE "<tr><th>Region</th><th>Units</th></tr>" TO REPORT-LINE
           WRITE REPORT-LINE
           ADD 1 TO WS-RECORDS
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 500
               MOVE "<tr><td>North</td><td>1204</td></tr>" TO REPORT-LINE
               WRITE REPORT-LINE
               ADD 1 TO WS-RECORDS
           END-PERFORM
           MOVE "</table>" TO REPORT-LINE
           WRITE REPORT-LINE
           ADD 1 TO WS-RECORDS
           MOVE 1 TO WS-PAGES
           CLOSE REPORT-FILE
           PERFORM CHECK-CLOSE
           PERFORM CLOCK-END
           PERFORM REPORT-RESULTS
           STOP RUN.

       CLOCK-START.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           COMPUTE WS-START =
               ((FUNCTION NUMVAL(WS-NOW-HH) * 3600)
              + (FUNCTION NUMVAL(WS-NOW-MM) * 60)
              +  FUNCTION NUMVAL(WS-NOW-SS)) * 100
              +  FUNCTION NUMVAL(WS-NOW-CS).

       CLOCK-END.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           COMPUTE WS-END =
               ((FUNCTION NUMVAL(WS-NOW-HH) * 3600)
              + (FUNCTION NUMVAL(WS-NOW-MM) * 60)
              +  FUNCTION NUMVAL(WS-NOW-SS)) * 100
              +  FUNCTION NUMVAL(WS-NOW-CS)
           COMPUTE WS-MS = (WS-END - WS-START) * 10
           IF WS-MS > 0
               COMPUTE WS-RATE = (WS-RECORDS * 1000) / WS-MS
           ELSE
               MOVE WS-RECORDS TO WS-RATE
           END-IF.

       CHECK-OPEN.
           IF WS-STATUS = "00"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL: OPEN OUTPUT status " WS-STATUS
           END-IF.

       CHECK-WRITE.
           IF WS-STATUS = "00"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL: WRITE status " WS-STATUS
           END-IF.

       CHECK-CLOSE.
           IF WS-STATUS = "00"
               ADD 1 TO WS-PASS
           ELSE
               ADD 1 TO WS-FAIL
               DISPLAY "FAIL: CLOSE status " WS-STATUS
           END-IF.

       REPORT-RESULTS.
           MOVE WS-RECORDS TO WS-SHOW-COUNT
           MOVE WS-MS      TO WS-SHOW-MS
           MOVE WS-RATE    TO WS-SHOW-RATE
           DISPLAY "================================================"
           DISPLAY "  REPORT TO VIEWER — RPTVWHT"
           DISPLAY "  ORGANIZATION IS HTML"
           DISPLAY "------------------------------------------------"
           DISPLAY "  forms exercised:"
           DISPLAY "    WRITE REPORT-LINE"
           DISPLAY "------------------------------------------------"
           DISPLAY "  records written : " WS-SHOW-COUNT
           DISPLAY "  pages produced  : " WS-PAGES
           DISPLAY "  elapsed (ms)    : " WS-SHOW-MS
           DISPLAY "  records/sec     : " WS-SHOW-RATE
           DISPLAY "  PASS            : " WS-PASS
           DISPLAY "  FAIL            : " WS-FAIL
           DISPLAY "================================================".
