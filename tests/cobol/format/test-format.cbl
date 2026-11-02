       IDENTIFICATION DIVISION.
      *> ============================================================
      *> TEST-FORMAT  (spec 109 S2)
      *> A numeric item declared FORMAT IS "name" LOCALE IS item,
      *> shown formatted by DISPLAY-free paths (MOVE of Format(),
      *> interpolated strings). Formats: number currency compact
      *> currency-short percent. Locales: pt-BR en-US fr-FR.
      *> The gap in "R$ 1,2 mi" is a no-break space (U+00A0), as in CLDR.
      *> Self-checking: a case prints only if it FAILS; one result
      *> block at the end gives the cases, the timing and the tally.
      *> ============================================================
       PROGRAM-ID. TEST-FORMAT.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-AMT                  PIC S9(13)V99 VALUE 1234567.89.
       01  WS-NEG                  PIC S9(13)V99 VALUE -1234.5.
       01  WS-RATIO                PIC 9V999 VALUE 0.425.
       01  WS-BIG                  PIC S9(13)V99 VALUE 1200000.
       01  WS-RES                  PIC X(60) VALUE SPACES.
       01  WS-EXP                  PIC X(60) VALUE SPACES.
       01  WS-LABEL                PIC X(40) VALUE SPACES.
       01  WS-I                    PIC 9(6)  VALUE 0.
       01  WS-TOT-RUN              PIC 9(4)  VALUE 0.
       01  WS-TOT-OK               PIC 9(4)  VALUE 0.
       01  WS-TOT-BAD              PIC 9(4)  VALUE 0.
       01  WS-T0                   PIC 9(8)  VALUE 0.
       01  WS-T1                   PIC 9(8)  VALUE 0.
       01  WS-TMS                  PIC 9(9)  VALUE 0.
       01  WS-LOOPS                PIC 9(6)  VALUE 5000.
       01  WS-RATE                 PIC 9(9)  VALUE 0.
       01  WS-HH                   PIC 9(2)  VALUE 0.
       01  WS-MM                   PIC 9(2)  VALUE 0.
       01  WS-SS                   PIC 9(2)  VALUE 0.
       01  WS-CC                   PIC 9(2)  VALUE 0.
       01  WS-MS0                  PIC 9(9)  VALUE 0.
       01  WS-MS1                  PIC 9(9)  VALUE 0.
       PROCEDURE DIVISION.
       MAIN-PARA.
           PERFORM CASES-NUMBER.
           PERFORM CASES-CURRENCY.
           PERFORM CASES-FRENCH.
           PERFORM CASES-OTHERS.
           PERFORM TIMING-LOOP.
           PERFORM SHOW-RESULTS.
           STOP RUN.

       CASES-NUMBER.
           MOVE "number en-US" TO WS-LABEL.
           MOVE "1,234,567.89" TO WS-EXP.
           MOVE WS-AMT::Format("number", "en-US") TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "number pt-BR" TO WS-LABEL.
           MOVE "1.234.567,89" TO WS-EXP.
           MOVE WS-AMT::Format("number", "pt-BR") TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "negative shows a leading minus" TO WS-LABEL.
           MOVE "-1,234.50" TO WS-EXP.
           MOVE WS-NEG::Format("number", "en-US") TO WS-RES.
           PERFORM CHECK-TEXT.

       CASES-FRENCH.
           MOVE "number fr-FR" TO WS-LABEL.
           MOVE "1 234 567,89" TO WS-EXP.
           MOVE WS-AMT::Format("number", "fr-FR") TO WS-RES.
           PERFORM CHECK-TEXT.

       CASES-CURRENCY.
           MOVE "currency BRL pt-BR" TO WS-LABEL.
           MOVE WS-AMT::Format("currency BRL", "pt-BR") TO WS-RES.
           MOVE "R$ 1.234.567,89" TO WS-EXP.
           PERFORM CHECK-TEXT.
           MOVE "currency BRL en-US" TO WS-LABEL.
           MOVE "R$1,234,567.89" TO WS-EXP.
           MOVE WS-AMT::Format("currency BRL", "en-US") TO WS-RES.
           PERFORM CHECK-TEXT.

       CASES-OTHERS.
           MOVE "percent pt-BR" TO WS-LABEL.
           MOVE "42,5%" TO WS-EXP.
           MOVE WS-RATIO::Format("percent", "pt-BR") TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "percent en-US" TO WS-LABEL.
           MOVE "42.5%" TO WS-EXP.
           MOVE WS-RATIO::Format("percent", "en-US") TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "compact pt-BR" TO WS-LABEL.
           MOVE "1,2 mi" TO WS-EXP.
           MOVE WS-BIG::Format("compact", "pt-BR") TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "currency-short BRL pt-BR" TO WS-LABEL.
           MOVE "R$ 1,2 mi" TO WS-EXP.
           MOVE WS-BIG::Format("currency-short BRL", "pt-BR") TO WS-RES.
           PERFORM CHECK-TEXT.
           MOVE "names ignore case" TO WS-LABEL.
           MOVE "42,5%" TO WS-EXP.
           MOVE WS-RATIO::Format("PERCENT", "PT-br") TO WS-RES.
           PERFORM CHECK-TEXT.

       TIMING-LOOP.
           ACCEPT WS-T0 FROM TIME.
           MOVE WS-T0(1:2) TO WS-HH.
           MOVE WS-T0(3:2) TO WS-MM.
           MOVE WS-T0(5:2) TO WS-SS.
           MOVE WS-T0(7:2) TO WS-CC.
           COMPUTE WS-MS0 = (WS-HH * 3600 + WS-MM * 60 + WS-SS) * 1000
               + WS-CC * 10.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-LOOPS
               MOVE WS-AMT::Format("currency BRL", "pt-BR") TO WS-RES
           END-PERFORM.
           ACCEPT WS-T1 FROM TIME.
           MOVE WS-T1(1:2) TO WS-HH.
           MOVE WS-T1(3:2) TO WS-MM.
           MOVE WS-T1(5:2) TO WS-SS.
           MOVE WS-T1(7:2) TO WS-CC.
           COMPUTE WS-MS1 = (WS-HH * 3600 + WS-MM * 60 + WS-SS) * 1000
               + WS-CC * 10.
           IF WS-MS1 >= WS-MS0
               COMPUTE WS-TMS = WS-MS1 - WS-MS0
           ELSE
               MOVE 0 TO WS-TMS
           END-IF.
           IF WS-TMS > 0
               COMPUTE WS-RATE = WS-LOOPS * 1000 / WS-TMS
           ELSE
               MOVE WS-LOOPS TO WS-RATE
           END-IF.

       CHECK-TEXT.
           ADD 1 TO WS-TOT-RUN.
           IF WS-RES = WS-EXP
               ADD 1 TO WS-TOT-OK
           ELSE
               ADD 1 TO WS-TOT-BAD
               DISPLAY "FAIL " WS-LABEL
               DISPLAY "  got      [" WS-RES "]"
               DISPLAY "  expected [" WS-EXP "]"
           END-IF.

       SHOW-RESULTS.
           DISPLAY "============================================".
           DISPLAY "TEST-FORMAT (spec 109 S2)".
           DISPLAY "Formats: number currency compact".
           DISPLAY "         currency-short percent".
           DISPLAY "Locales: pt-BR en-US fr-FR (case-insensitive)".
           DISPLAY "--------------------------------------------".
           DISPLAY "CASES RUN       : " WS-TOT-RUN.
           DISPLAY "CASES PASSED    : " WS-TOT-OK.
           DISPLAY "CASES FAILED    : " WS-TOT-BAD.
           DISPLAY "PHASE TIMING    : " WS-LOOPS
               " Format() calls in " WS-TMS " ms".
           DISPLAY "THROUGHPUT      : " WS-RATE " calls/sec".
           IF WS-TOT-BAD = ZERO
               DISPLAY "OVERALL RESULT  : PASS"
           ELSE
               DISPLAY "OVERALL RESULT  : FAIL"
           END-IF.
           DISPLAY "============================================".
