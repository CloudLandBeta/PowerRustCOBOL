      *> DESKTOP-WASM-001 T24 console-parity fixture "calls": CALL into
      *> Common Code BY REFERENCE and BY CONTENT, CANCEL, a
      *> nested program, sections, SORT with INPUT and OUTPUT PROCEDUREs
      *> (RELEASE, RETURN), EXIT PROGRAM and GOBACK, on both paths.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PARITYCALLS.
       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.
           SELECT WORK-FILE ASSIGN TO "work".
       DATA DIVISION.
       FILE SECTION.
       SD WORK-FILE.
       01 W-REC.
          05 W-CITY      PIC X(10).
          05 W-POP       PIC 9(7).
       WORKING-STORAGE SECTION.
       01 WS-TOTAL       PIC 9(5) VALUE 0.
       01 WS-STEP        PIC 9(3) VALUE 7.
       01 WS-KEPT        PIC 9(3) VALUE 7.
       01 WS-TIMES       PIC 9(3).
       01 WS-EOF         PIC X.
       01 WS-I           PIC 9.
       01 WS-CITIES.
          05 FILLER      PIC X(17) VALUE "LISBON    0545000".
          05 FILLER      PIC X(17) VALUE "PORTO     0232000".
          05 FILLER      PIC X(17) VALUE "BRAGA     0193000".
          05 FILLER      PIC X(17) VALUE "COIMBRA   0140000".
       01 WS-ROWS REDEFINES WS-CITIES.
          05 WS-ROW OCCURS 4 TIMES.
             10 WS-CITY  PIC X(10).
             10 WS-POP   PIC 9(7).
       PROCEDURE DIVISION.
       MAIN-SECTION SECTION.
       MAIN-PARA.
           DISPLAY "parity calls fixture"
           PERFORM 3 TIMES
               *> PARITYCOUNTER: adds the step, times how many calls it has had,
               *> to the total; it keeps that count until it is CANCELled.
               *> Uses:   WS-TOTAL (0, then 7, then 21) BY REFERENCE; WS-STEP (7)
               *>         BY CONTENT, so the 999 it moves into its copy stays there.
               *> Result: WS-TOTAL becomes 7, 21, then 42; WS-STEP stays 7.
               CALL "PARITYCOUNTER" USING BY REFERENCE WS-TOTAL
                                          BY CONTENT WS-STEP
           END-PERFORM
           DISPLAY "counter total " WS-TOTAL " step " WS-STEP
           CANCEL "PARITYCOUNTER"
           *> PARITYCOUNTER, after the CANCEL: it starts again from its initial
           *> state, its count of calls back at 0.
           *> Uses:   WS-TOTAL (42) and WS-STEP (7), both BY REFERENCE.
           *> Result: WS-TOTAL becomes 49 (42 + 7 x 1); WS-STEP becomes 999.
           CALL "PARITYCOUNTER" USING WS-TOTAL WS-STEP
           DISPLAY "after cancel " WS-TOTAL
           *> PARITYDOUBLE: doubles the item it is given.
           *> Uses:   WS-KEPT (7) BY REFERENCE.
           *> Result: WS-KEPT becomes 14.
           CALL "PARITYDOUBLE" USING BY REFERENCE WS-KEPT
           DISPLAY "doubled " WS-KEPT
           *> PARITYINNER (the contained program): fills the item it is given.
           *> Uses:   WS-TIMES (no value yet).
           *> Result: WS-TIMES becomes 321.
           CALL "PARITYINNER" USING WS-TIMES
           DISPLAY "nested " WS-TIMES
           PERFORM REPORT-SECTION
           SORT WORK-FILE ON ASCENDING KEY W-POP
               INPUT PROCEDURE IS GIVE-CITIES
               OUTPUT PROCEDURE IS TAKE-CITIES
           DISPLAY "end"
           STOP RUN.

       REPORT-SECTION SECTION.
       REPORT-HEAD.
           DISPLAY "report section".
       REPORT-BODY.
           DISPLAY "report body " WS-TOTAL.

       GIVE-CITIES SECTION.
       GIVE-ALL.
           PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 4
               MOVE WS-CITY(WS-I) TO W-CITY
               MOVE WS-POP(WS-I) TO W-POP
               RELEASE W-REC
           END-PERFORM.

       TAKE-CITIES SECTION.
       TAKE-ALL.
           MOVE "N" TO WS-EOF
           PERFORM UNTIL WS-EOF = "Y"
               RETURN WORK-FILE
                   AT END MOVE "Y" TO WS-EOF
                   NOT AT END DISPLAY "sorted " W-CITY " " W-POP
               END-RETURN
           END-PERFORM.

       IDENTIFICATION DIVISION.
       PROGRAM-ID. PARITYINNER.
       DATA DIVISION.
       LINKAGE SECTION.
       01 LK-TIMES       PIC 9(3).
       PROCEDURE DIVISION USING LK-TIMES.
           MOVE 321 TO LK-TIMES
           EXIT PROGRAM.
       END PROGRAM PARITYINNER.
       END PROGRAM PARITYCALLS.
