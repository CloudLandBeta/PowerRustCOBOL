       IDENTIFICATION DIVISION.
       PROGRAM-ID. UTF8-BASICS.
      *> Spec 077 - UTF-8 data (PIC U, USAGE UTF-8, BYTE-LENGTH), UTF-8
      *> literals and escapes, and the U-functions (AC12, AC13, AC14).
      *> Each case is named; one result block at the end.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-U       PIC U(5) VALUE U"Ação!".
       01  WS-B       PIC U BYTE-LENGTH 5.
       01  WS-T       PIC X(10) VALUE "Aç€😀".
       01  WS-S       PIC X(10).
       01  WS-C       PIC U(1).
       01  G.
           05  G-N    PIC N(1) VALUE N"é".
       01  G-R REDEFINES G PIC X(2).
       01  WS-L       PIC 9(4).
       01  WS-I       PIC 9(7).
       01  WS-OPS     PIC 9(7) VALUE 100000.
       01  WS-SUM     PIC 9(9).
       01  WS-PASS    PIC 9(3) VALUE 0.
       01  WS-FAIL    PIC 9(3) VALUE 0.
       01  WS-T0      PIC 9(9).
       01  WS-MS      PIC 9(9).
       01  WS-BULK-MS PIC 9(9).
       01  WS-RATE    PIC 9(9).
       01  WS-NOW     PIC X(21).
       PROCEDURE DIVISION.
       MAIN-PARA.
      *>   U01, U02, U03 - AC12: characters, storage, whole characters.
           MOVE FUNCTION ULENGTH(WS-U) TO WS-L
           IF WS-L = 5 AND FUNCTION LENGTH(WS-U) = 5 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U01 ULENGTH " WS-L
           END-IF
           MOVE FUNCTION BYTE-LENGTH(WS-U) TO WS-L
           IF WS-L = 20 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U02 BYTE-LENGTH " WS-L
           END-IF
           MOVE U"Configuração" TO WS-U
           IF WS-U = U"Confi" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U03 [" WS-U "]"
           END-IF
      *>   U04 - BYTE-LENGTH 5 keeps the whole characters that fit.
           MOVE "Ação" TO WS-B
           IF WS-B = U"Açã" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U04 [" WS-B "]"
           END-IF
      *>   U05, U06 - AC13: UX"C3A7" and the \u escape are both ç.
           MOVE UX"C3A7" TO WS-C
           IF WS-C = U"ç" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U05 [" WS-C "]"
           END-IF
           IF U"ç" = U"ç" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U06 escape"
           END-IF
      *>   U07 to U12 - AC14 over the alphanumeric text "Aç€😀".
           IF FUNCTION ULENGTH(FUNCTION TRIM(WS-T)) = 4
               ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U07 ULENGTH"
           END-IF
           IF FUNCTION UPOS(WS-T, 3) = 4 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U08 UPOS"
           END-IF
           MOVE FUNCTION USUBSTR(WS-T, 2, 2) TO WS-S
           IF WS-S = "ç€" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U09 [" WS-S "]"
           END-IF
           IF FUNCTION UWIDTH(WS-T, 4) = 4 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U10 UWIDTH"
           END-IF
           IF FUNCTION USUPPLEMENTARY(WS-T) = 7 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U11 USUPPLEMENTARY"
           END-IF
           IF FUNCTION UVALID(WS-T) = 0 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U12 UVALID"
           END-IF
      *>   U13 - UVALID over bytes that are not UTF-8: X'00E9', the
      *>   national image of "é", read through a REDEFINES.
           IF FUNCTION UVALID(G-R) = 2 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U13 UVALID bad byte"
           END-IF
      *>   Bulk: ULENGTH and UPOS over the same text.
           PERFORM NOW-MS
           MOVE WS-MS TO WS-T0
           MOVE 0 TO WS-SUM
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-OPS
               COMPUTE WS-SUM = WS-SUM + FUNCTION ULENGTH(WS-T)
                              + FUNCTION UPOS(WS-T, 4)
           END-PERFORM
           PERFORM NOW-MS
           COMPUTE WS-BULK-MS = WS-MS - WS-T0
      *>   "Aç€😀" fills all ten bytes of WS-T: ULENGTH is 4 and the
      *>   fourth character starts at byte 7, so each pass adds 11.
           IF WS-SUM = 11 * WS-OPS ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL U14 bulk " WS-SUM
           END-IF
           IF WS-BULK-MS = 0 MOVE 1 TO WS-BULK-MS END-IF
           COMPUTE WS-RATE = WS-OPS * 1000 / WS-BULK-MS
           DISPLAY "=== UTF-8 BASICS RESULT ==="
           DISPLAY "cases: U01 ULENGTH = LENGTH = 5 for PIC U(5);"
           DISPLAY "  U02 BYTE-LENGTH(PIC U(5)) = 20;"
           DISPLAY "  U03 MOVE U""Configuração"" keeps ""Confi"";"
           DISPLAY "  U04 PIC U BYTE-LENGTH 5 keeps ""Açã"";"
           DISPLAY "  U05 UX""C3A7"" = ç; U06 U""ç"" escape;"
           DISPLAY "  over ""Aç€😀"": U07 ULENGTH 4; U08 UPOS(3) 4;"
           DISPLAY "  U09 USUBSTR(2,2) ç€; U10 UWIDTH(4) 4;"
           DISPLAY "  U11 USUPPLEMENTARY 7; U12 UVALID 0;"
           DISPLAY "  U13 UVALID over X'00E9' = 2;"
           DISPLAY "  U14 bulk ULENGTH + UPOS x " WS-OPS
           DISPLAY "rows: bulk iterations " WS-OPS " sum " WS-SUM
           DISPLAY "bulk: " WS-BULK-MS " ms, " WS-RATE " iterations/s"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.
       NOW-MS.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           COMPUTE WS-MS = (FUNCTION NUMVAL(WS-NOW(9:2)) * 3600
                         + FUNCTION NUMVAL(WS-NOW(11:2)) * 60
                         + FUNCTION NUMVAL(WS-NOW(13:2))) * 1000
                         + FUNCTION NUMVAL(WS-NOW(15:2)) * 10.
