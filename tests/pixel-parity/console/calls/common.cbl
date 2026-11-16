      *> Common Code of the "calls" console-parity fixture (T24): a
      *> counter that keeps its own state between CALLs until CANCEL.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PARITYCOUNTER.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01 WS-CALLS       PIC 9(3) VALUE 0.
       LINKAGE SECTION.
       01 LK-TOTAL       PIC 9(5).
       01 LK-STEP        PIC 9(3).
       PROCEDURE DIVISION USING LK-TOTAL LK-STEP.
           ADD 1 TO WS-CALLS
           COMPUTE LK-TOTAL = LK-TOTAL + LK-STEP * WS-CALLS
           MOVE 999 TO LK-STEP
           GOBACK.
       END PROGRAM PARITYCOUNTER.
