       IDENTIFICATION DIVISION.
       PROGRAM-ID. NUMEDIT.
      *> Self-checking suite for numeric-edited PICTUREs:
      *>   Z zero-suppression, * check-protection, floating/fixed $,
      *>   floating sign, comma + decimal point insertion, CR / DB.
       ENVIRONMENT DIVISION.
       DATA DIVISION.
       WORKING-STORAGE SECTION.
       01  TEST-COUNTERS.
           05 TESTS-RUN      PIC 9(4) VALUE 0.
           05 TESTS-PASSED   PIC 9(4) VALUE 0.
           05 TESTS-FAILED   PIC 9(4) VALUE 0.

       01  SRC.
           05 S-POS          PIC 9(6)V99  VALUE 1234.50.
           05 S-SMALL        PIC 9(6)V99  VALUE 5.00.
           05 S-ZERO         PIC 9(6)V99  VALUE 0.
           05 S-CENTS        PIC 9(6)V99  VALUE 12.34.
           05 S-NEG          PIC S9(4)V99 VALUE -12.30.
           05 S-POSV         PIC S9(4)V99 VALUE 12.30.

       01  E-ZSUP            PIC ZZZ,ZZ9.99.
       01  E-ZSUP0           PIC ZZZ,ZZ9.99.
       01  E-FLOAT           PIC $$$,$$9.99.
       01  E-FLOATS          PIC $$$,$$9.99.
       01  E-FIXED           PIC $9,999.99.
       01  E-STAR            PIC ***,**9.99.
       01  E-SIGN            PIC ----9.99.
       01  E-SIGNP           PIC ----9.99.
       01  E-CR              PIC 9(6).99CR.
       01  E-CRP             PIC 9(6).99CR.
       01  E-DB              PIC 9(6).99DB.

       PROCEDURE DIVISION.
       MAIN-PARA.
           DISPLAY "NUMERIC-EDITED PICTURE TEST SUITE".
           DISPLAY "---------------------------------".

           MOVE S-POS   TO E-ZSUP
           PERFORM CHECK-ZSUP
           MOVE S-ZERO  TO E-ZSUP0
           PERFORM CHECK-ZSUP0
           MOVE S-POS   TO E-FLOAT
           PERFORM CHECK-FLOAT
           MOVE S-SMALL TO E-FLOATS
           PERFORM CHECK-FLOATS
           MOVE S-POS   TO E-FIXED
           PERFORM CHECK-FIXED
           MOVE S-CENTS TO E-STAR
           PERFORM CHECK-STAR
           MOVE S-NEG   TO E-SIGN
           PERFORM CHECK-SIGN
           MOVE S-POSV  TO E-SIGNP
           PERFORM CHECK-SIGNP
           MOVE S-NEG   TO E-CR
           PERFORM CHECK-CR
           MOVE S-POSV  TO E-CRP
           PERFORM CHECK-CRP
           MOVE S-NEG   TO E-DB
           PERFORM CHECK-DB

           DISPLAY "---------------------------------".
           DISPLAY "TESTS RUN    : " TESTS-RUN.
           DISPLAY "TESTS PASSED : " TESTS-PASSED.
           DISPLAY "TESTS FAILED : " TESTS-FAILED.
           IF TESTS-FAILED = ZERO
               DISPLAY "RESULT       : PASS"
           ELSE
               DISPLAY "RESULT       : FAIL"
           END-IF.
           STOP RUN.

       CHECK-ZSUP.
           ADD 1 TO TESTS-RUN.
           IF E-ZSUP = "  1,234.50"
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T001 Z-SUPPRESSION"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T001 Z-SUPPRESSION [" E-ZSUP "]"
           END-IF.

       CHECK-ZSUP0.
           ADD 1 TO TESTS-RUN.
           IF E-ZSUP0 = "      0.00"
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T002 Z-SUPPRESSION ZERO"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T002 Z-SUPPRESSION ZERO [" E-ZSUP0 "]"
           END-IF.

       CHECK-FLOAT.
           ADD 1 TO TESTS-RUN.
           IF E-FLOAT = " $1,234.50"
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T003 FLOATING DOLLAR"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T003 FLOATING DOLLAR [" E-FLOAT "]"
           END-IF.

       CHECK-FLOATS.
           ADD 1 TO TESTS-RUN.
           IF E-FLOATS = "     $5.00"
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T004 FLOATING DOLLAR SMALL"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T004 FLOATING DOLLAR SMALL [" E-FLOATS "]"
           END-IF.

       CHECK-FIXED.
           ADD 1 TO TESTS-RUN.
           IF E-FIXED = "$1,234.50"
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T005 FIXED DOLLAR"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T005 FIXED DOLLAR [" E-FIXED "]"
           END-IF.

       CHECK-STAR.
           ADD 1 TO TESTS-RUN.
           IF E-STAR = "*****12.34"
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T006 CHECK PROTECTION"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T006 CHECK PROTECTION [" E-STAR "]"
           END-IF.

       CHECK-SIGN.
           ADD 1 TO TESTS-RUN.
           IF E-SIGN = "  -12.30"
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T007 FLOATING SIGN NEG"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T007 FLOATING SIGN NEG [" E-SIGN "]"
           END-IF.

       CHECK-SIGNP.
           ADD 1 TO TESTS-RUN.
           IF E-SIGNP = "   12.30"
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T008 FLOATING SIGN POS"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T008 FLOATING SIGN POS [" E-SIGNP "]"
           END-IF.

       CHECK-CR.
           ADD 1 TO TESTS-RUN.
           IF E-CR = "000012.30CR"
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T009 CR ON NEGATIVE"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T009 CR ON NEGATIVE [" E-CR "]"
           END-IF.

       CHECK-CRP.
           ADD 1 TO TESTS-RUN.
           IF E-CRP = "000012.30  "
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T010 CR BLANK ON POSITIVE"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T010 CR BLANK ON POSITIVE [" E-CRP "]"
           END-IF.

       CHECK-DB.
           ADD 1 TO TESTS-RUN.
           IF E-DB = "000012.30DB"
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS T011 DB ON NEGATIVE"
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL T011 DB ON NEGATIVE [" E-DB "]"
           END-IF.
