       IDENTIFICATION DIVISION.
       PROGRAM-ID. NAT-BASICS.
      *> Spec 077 - national data (PIC N, USAGE NATIONAL): storage,
      *> literals, moves, comparisons, string verbs, figuratives and
      *> INITIALIZE (AC1, AC2, AC3, AC5, AC6, AC7). Each case is named;
      *> a failing one prints what it saw. One result block at the end.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  WS-MSG     PIC N(30) VALUE N"Configuração concluída – ok".
       01  G1.
           05  G1-N   PIC N(3) VALUE N"Açã".
           05  G1-X   PIC X(2) VALUE "xy".
       01  G1-R REDEFINES G1 PIC X(8).
       01  G2.
           05  G2-N   PIC N(3).
           05  G2-X   PIC X(2).
       01  G3.
           05  G3-N   PIC N(3) VALUE N"ABC".
           05  G3-X   PIC X(2) VALUE "xy".
       01  G3-R REDEFINES G3 PIC X(8).
       01  WS-N10     PIC N(10).
       01  WS-X10     PIC X(10).
       01  WS-A       PIC N(4) VALUE N"Ação".
       01  WS-B       PIC N(4) VALUE N"Pão".
       01  WS-R       PIC N(10).
       01  WS-P       PIC 9(3).
       01  WS-S       PIC N(13) VALUE N"maçã,pêra,uva".
       01  WS-1       PIC N(5).
       01  WS-2       PIC N(5).
       01  WS-3       PIC N(5).
       01  WS-C1      PIC 9(3).
       01  WS-T       PIC N(13) VALUE N"ação, coração".
       01  WS-H       PIC N(1).
       01  WS-J       PIC N(5) JUSTIFIED RIGHT.
       01  GI.
           05  GI-N   PIC N(3) VALUE N"abc".
           05  GI-X   PIC X(3) VALUE "ghi".
       01  WS-L       PIC 9(4).
       01  WS-I       PIC 9(7).
       01  WS-OPS     PIC 9(7) VALUE 100000.
       01  WS-COUNT   PIC 9(7).
       01  WS-PASS    PIC 9(3) VALUE 0.
       01  WS-FAIL    PIC 9(3) VALUE 0.
       01  WS-T0      PIC 9(9).
       01  WS-MS      PIC 9(9).
       01  WS-BULK-MS PIC 9(9).
       01  WS-RATE    PIC 9(9).
       01  WS-NOW     PIC X(21).
       PROCEDURE DIVISION.
       MAIN-PARA.
      *>   C01, C02 - AC1: characters and bytes.
           MOVE FUNCTION LENGTH(WS-MSG) TO WS-L
           IF WS-L = 30 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C01 LENGTH " WS-L
           END-IF
           MOVE FUNCTION BYTE-LENGTH(WS-MSG) TO WS-L
           IF WS-L = 60 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C02 BYTE-LENGTH " WS-L
           END-IF
      *>   C03 - the value: 27 characters, padded with spaces.
           IF WS-MSG = N"Configuração concluída – ok"
               ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C03 [" WS-MSG "]"
           END-IF
      *>   C04, C05, C06 - AC2: group size, REDEFINES image, group move.
           MOVE FUNCTION BYTE-LENGTH(G1) TO WS-L
           IF WS-L = 8 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C04 group " WS-L
           END-IF
           IF G3-R = X"0041004200437879" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C05 REDEFINES image"
           END-IF
           MOVE G1 TO G2
           IF G2-N = N"Açã" AND G2-X = "xy" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C06 [" G2-N "]"
           END-IF
      *>   C07 - AC3: a national hex literal names code points.
           IF WS-A = NX"004100E700E3006F" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C07 NX literal"
           END-IF
      *>   C08, C09 - AC5: to PIC X keeps whole characters, and back.
           MOVE N"Configuração" TO WS-N10
           MOVE WS-N10 TO WS-X10
           IF WS-X10 = "Configura " ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C08 [" WS-X10 "]"
           END-IF
           MOVE WS-X10 TO WS-N10
           IF WS-N10 = N"Configura" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C09 [" WS-N10 "]"
           END-IF
      *>   C10, C11 - AC6: comparison with PIC X text; code-point order.
           IF WS-A = "Ação" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C10"
           END-IF
           IF N"Ação" > N"Açb" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C11 order"
           END-IF
      *>   C12 - AC7: INSPECT counts characters.
           MOVE 0 TO WS-C1
           INSPECT WS-T TALLYING WS-C1 FOR ALL N"ç"
           IF WS-C1 = 2 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C12 tally " WS-C1
           END-IF
      *>   C13 - AC7: STRING into a national receiver, POINTER by character.
           MOVE 1 TO WS-P
           STRING WS-A DELIMITED BY SIZE N"-" DELIMITED BY SIZE
                  WS-B DELIMITED BY SPACE INTO WS-R WITH POINTER WS-P
           IF WS-R = N"Ação-Pão" AND WS-P = 9 ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C13 [" WS-R "] " WS-P
           END-IF
      *>   C14 - AC7: UNSTRING by N",", COUNT IN by character.
           UNSTRING WS-S DELIMITED BY N","
               INTO WS-1 COUNT IN WS-C1 WS-2 WS-3
           IF WS-1 = N"maçã" AND WS-2 = N"pêra" AND WS-3 = N"uva"
              AND WS-C1 = 4
               ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C14 [" WS-1 "] " WS-C1
           END-IF
      *>   C15 - HIGH-VALUE in a national item is U+FFFF.
           MOVE HIGH-VALUE TO WS-H
           IF WS-H = NX"FFFF" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C15 HIGH-VALUE"
           END-IF
      *>   C16 - JUSTIFIED RIGHT by character.
           MOVE "Ação" TO WS-J
           IF WS-J = N" Ação" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C16 [" WS-J "]"
           END-IF
      *>   C17 - INITIALIZE ... REPLACING NATIONAL DATA BY.
           INITIALIZE GI REPLACING NATIONAL DATA BY N"çã"
           IF GI-N = N"çã" AND GI-X = "ghi" ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C17 [" GI-N "]"
           END-IF
      *>   Bulk: MOVE into PIC N, then INSPECT it, WS-OPS times.
           PERFORM NOW-MS
           MOVE WS-MS TO WS-T0
           MOVE 0 TO WS-COUNT
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > WS-OPS
               MOVE N"ação, coração" TO WS-T
               INSPECT WS-T TALLYING WS-COUNT FOR ALL N"ç"
           END-PERFORM
           PERFORM NOW-MS
           COMPUTE WS-BULK-MS = WS-MS - WS-T0
           IF WS-COUNT = 2 * WS-OPS ADD 1 TO WS-PASS
           ELSE ADD 1 TO WS-FAIL DISPLAY "FAIL C18 bulk " WS-COUNT
           END-IF
           IF WS-BULK-MS = 0 MOVE 1 TO WS-BULK-MS END-IF
           COMPUTE WS-RATE = WS-OPS * 1000 / WS-BULK-MS
           DISPLAY "=== NATIONAL BASICS RESULT ==="
           DISPLAY "cases: C01 LENGTH(PIC N(30))=30; C02 BYTE-LENGTH=60;"
           DISPLAY "  C03 VALUE N""...""; C04 group N(3)+X(2)=8 bytes;"
           DISPLAY "  C05 REDEFINES PIC X(8) = UTF-16 image;"
           DISPLAY "  C06 group MOVE restores; C07 NX""..."" literal;"
           DISPLAY "  C08 MOVE PIC N -> PIC X whole characters;"
           DISPLAY "  C09 and back; C10 PIC N = PIC X literal;"
           DISPLAY "  C11 code-point order; C12 INSPECT TALLYING ALL N""ç"";"
           DISPLAY "  C13 STRING ... WITH POINTER into PIC N;"
           DISPLAY "  C14 UNSTRING DELIMITED BY N"","" COUNT IN;"
           DISPLAY "  C15 HIGH-VALUE = NX""FFFF""; C16 JUSTIFIED RIGHT;"
           DISPLAY "  C17 INITIALIZE REPLACING NATIONAL DATA;"
           DISPLAY "  C18 bulk MOVE + INSPECT x " WS-OPS
           DISPLAY "rows: bulk iterations " WS-OPS " tallied " WS-COUNT
           DISPLAY "bulk: " WS-BULK-MS " ms, " WS-RATE " iterations/s"
           DISPLAY "PASS " WS-PASS " FAIL " WS-FAIL
           STOP RUN.
       NOW-MS.
           MOVE FUNCTION CURRENT-DATE TO WS-NOW
           COMPUTE WS-MS = (FUNCTION NUMVAL(WS-NOW(9:2)) * 3600
                         + FUNCTION NUMVAL(WS-NOW(11:2)) * 60
                         + FUNCTION NUMVAL(WS-NOW(13:2))) * 1000
                         + FUNCTION NUMVAL(WS-NOW(15:2)) * 10.
