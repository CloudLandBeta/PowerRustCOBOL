       IDENTIFICATION DIVISION.
       PROGRAM-ID. NAT-FUNCTIONS.
      *> Spec 077 AC4 - NATIONAL-OF and DISPLAY-OF across the code pages
      *> RustCOBOL converts (UTF-8 1208, WINDOWS-1252 1252, ISO-8859-1
      *> 819), IBM's substitution character, and Unicode UPPER-CASE /
      *> LOWER-CASE for national data. One result block at the end.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-X       PIC X(10).
       01  WS-X1      PIC X(1).
       01  WS-N       PIC N(10).
       01  WS-CP      PIC X(12) VALUE "WINDOWS-1252".
       01  WS-L       PIC 9(4).
       01  WS-I       PIC 9(7).
       01  WS-OPS     PIC 9(7) VALUE 50000.
       01  WS-PASS    PIC 9(3) VALUE 0.
       01  WS-FAIL    PIC 9(3) VALUE 0.
       01  WS-T0      PIC 9(9).
       01  WS-MS      PIC 9(9).
       01  WS-BULK-MS PIC 9(9).
       01  WS-RATE    PIC 9(9).
       01  WS-NOW     PIC X(21).
       PROCEDURE DIVISION.
       MAIN-PARA.
      *>   F01 - UTF-8 round trip (the default code page).
           MOVE FUNCTION DISPLAY-OF(FUNCTION NATIONAL-OF("Ação"))
             TO WS-X
           IF WS-X = "Ação" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL F01 [" WS-X "]"
           END-IF
      *>   F02 - Windows-1252: one byte a character, and back by name
      *>   held in a data item.
           MOVE FUNCTION DISPLAY-OF(N"Ação", 1252) TO WS-X
           MOVE FUNCTION BYTE-LENGTH(FUNCTION DISPLAY-OF(N"Ação", 1252))
             TO WS-L
           MOVE FUNCTION NATIONAL-OF(WS-X, WS-CP) TO WS-N
           IF WS-L = 4 AND WS-N = N"Ação" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL F02 " WS-L " [" WS-N "]"
           END-IF
      *>   F03 - ISO-8859-1 by CCSID: Latin-1 letters convert.
           MOVE FUNCTION DISPLAY-OF(N"ç", 819) TO WS-X1
           MOVE FUNCTION NATIONAL-OF(WS-X1, "ISO-8859-1") TO WS-N
           IF WS-N = N"ç" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL F03 [" WS-N "]"
           END-IF
      *>   F04 - a character ISO-8859-1 cannot hold becomes X'7F'.
           MOVE FUNCTION DISPLAY-OF(N"€", 819) TO WS-X1
           IF WS-X1 = X"7F" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL F04 substitution"
           END-IF
      *>   F05, F06 - Unicode case mapping for national data.
           MOVE FUNCTION UPPER-CASE(N"ação") TO WS-N
           IF WS-N = N"AÇÃO" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL F05 [" WS-N "]"
           END-IF
           MOVE FUNCTION LOWER-CASE(N"ÉLAN") TO WS-N
           IF WS-N = N"élan" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL F06 [" WS-N "]"
           END-IF
      *>   Bulk: Windows-1252 round trips.
           PERFORM NOW-MS
           MOVE WS-MS TO WS-T0
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-OPS
               MOVE FUNCTION DISPLAY-OF(N"Coração", 1252) TO WS-X
               MOVE FUNCTION NATIONAL-OF(WS-X, 1252) TO WS-N
           END-PERFORM
           PERFORM NOW-MS
           COMPUTE WS-BULK-MS = WS-MS - WS-T0
           IF WS-N = N"Coração" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL F07 [" WS-N "]"
           END-IF
           IF WS-BULK-MS = 0 MOVE 1 TO WS-BULK-MS END-IF
           COMPUTE WS-RATE = WS-OPS * 1000 / WS-BULK-MS
           DISPLAY "=== NATIONAL FUNCTIONS RESULT ==="
           DISPLAY "cases: F01 DISPLAY-OF(NATIONAL-OF(""Ação"")) UTF-8;"
           DISPLAY "  F02 DISPLAY-OF(N, 1252) is 4 bytes, NATIONAL-OF"
           DISPLAY "      (x, WS-CP holding ""WINDOWS-1252"");"
           DISPLAY "  F03 ISO-8859-1 by CCSID 819 and by name;"
           DISPLAY "  F04 N""€"" into 819 = X""7F"";"
           DISPLAY "  F05 UPPER-CASE(N""ação""); F06 LOWER-CASE(N""ÉLAN"");"
           DISPLAY "  F07 bulk 1252 round trips x " WS-OPS
           DISPLAY "rows: round trips " WS-OPS
           DISPLAY "bulk: " WS-BULK-MS " ms, " WS-RATE " round trips/s"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.
       NOW-MS.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           COMPUTE WS-MS = (FUNCTION NUMVAL(WS-NOW(9:2)) * 3600
                         + FUNCTION NUMVAL(WS-NOW(11:2)) * 60
                         + FUNCTION NUMVAL(WS-NOW(13:2))) * 1000
                         + FUNCTION NUMVAL(WS-NOW(15:2)) * 10.
