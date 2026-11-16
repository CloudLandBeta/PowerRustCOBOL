      *> Common Code of the "calls" console-parity fixture (T24): a
      *> program that changes the item it is given.
       IDENTIFICATION DIVISION.
       PROGRAM-ID. PARITYDOUBLE.
       DATA DIVISION.
       LINKAGE SECTION.
       01 LK-N           PIC 9(3).
       PROCEDURE DIVISION USING LK-N.
           MULTIPLY 2 BY LK-N
           GOBACK.
       END PROGRAM PARITYDOUBLE.
