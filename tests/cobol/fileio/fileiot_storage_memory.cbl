       IDENTIFICATION DIVISION.
       PROGRAM-ID. FILEIOTM.

      *>---------------------------------------------------------------*
      *> FILEIOTM                                                    *
      *>                                                               *
      *> COBOL-85 style file I/O unit test for:                        *
      *>   - ORGANIZATION IS SEQUENTIAL                                *
      *>   - ORGANIZATION IS LINE SEQUENTIAL                           *
      *>   - ORGANIZATION IS INDEXED                                   *
      *>   - Storage variant: 3. STORAGE IS MEMORY without compressi *
      *>                                                               *
      *> The indexed-file section tests:                               *
      *>   - alphanumeric primary key                                  *
      *>   - alphabetic-only primary key content                       *
      *>   - numeric DISPLAY primary key                               *
      *>   - uppercase, lowercase, and mixed-case key content           *
      *>   - alternate keys with duplicates                             *
      *>   - alternate keys without duplicates                          *
      *>   - duplicate primary key error                                *
      *>   - duplicate alternate-key error                              *
      *>   - random READ by primary key                                 *
      *>   - random READ by alternate key                               *
      *>   - sequential READ NEXT after START                           *
      *>   - START equal / greater-or-equal                             *
      *>   - START invalid-key path                                     *
      *>   - REWRITE                                                   *
      *>   - DELETE                                                    *
      *>   - invalid-key READ after DELETE                              *
      *>                                                               *
      *> Performance/profile test:                                     *
      *>   - creates 1,000,000 indexed records                          *
      *>   - primary key is 40 bytes                                    *
      *>   - record size is 1024 bytes                                  *
      *>   - file is kept on disk after test                            *
      *>   - file path and timing statistics are displayed              *
      *>---------------------------------------------------------------*

       ENVIRONMENT DIVISION.
       INPUT-OUTPUT SECTION.
       FILE-CONTROL.

           SELECT SEQ-FILE
               ASSIGN TO "/tmp/seq-test.dat"
               ORGANIZATION IS SEQUENTIAL
               FILE STATUS IS SEQ-STATUS.

           SELECT LINESEQ-FILE
               ASSIGN TO
               "/tmp/lineseq-test.txt"
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS LINESEQ-STATUS.

           SELECT IDX-MAIN-FILE
               ASSIGN TO "/tmp/idx-main.dat"
               ORGANIZATION IS INDEXED
               STORAGE IS MEMORY
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS IDX-PRIMARY-KEY
               ALTERNATE RECORD KEY IS IDX-ALT-UPPER
                   WITH DUPLICATES
               ALTERNATE RECORD KEY IS IDX-ALT-LOWER
               ALTERNATE RECORD KEY IS IDX-ALT-MIXED
                   WITH DUPLICATES
               FILE STATUS IS IDX-STATUS.

           SELECT IDX-NUM-FILE
               ASSIGN TO "/tmp/idx-numeric.dat"
               ORGANIZATION IS INDEXED
               STORAGE IS MEMORY
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS NUM-KEY
               FILE STATUS IS NUM-STATUS.

           SELECT IDX-ALPHA-FILE
               ASSIGN TO "/tmp/idx-alpha.dat"
               ORGANIZATION IS INDEXED
               STORAGE IS MEMORY
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS ALPHA-KEY
               FILE STATUS IS ALPHA-STATUS.

           SELECT IDX-PERF-FILE
               ASSIGN TO "/tmp/idx-perf-uuid-1m.dat"
               ORGANIZATION IS INDEXED
               STORAGE IS MEMORY
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS PERF-KEY
               FILE STATUS IS PERF-STATUS.

       DATA DIVISION.
       FILE SECTION.

       FD  SEQ-FILE.
       01  SEQ-REC.
           05 SEQ-ID                    PIC 9(5).
           05 SEQ-NAME                  PIC X(20).
           05 SEQ-AMOUNT                PIC S9(7)V99.
           05 SEQ-FILLER                PIC X(20).

       FD  LINESEQ-FILE.
       01  LINESEQ-REC                  PIC X(80).

       FD  IDX-MAIN-FILE.
       01  IDX-REC.
           05 IDX-PRIMARY-KEY           PIC X(40).
           05 IDX-ALT-UPPER             PIC X(20).
           05 IDX-ALT-LOWER             PIC X(20).
           05 IDX-ALT-MIXED             PIC X(20).
           05 IDX-PAYLOAD               PIC X(120).

       FD  IDX-NUM-FILE.
       01  NUM-REC.
           05 NUM-KEY                   PIC 9(8).
           05 NUM-TEXT                  PIC X(40).

       FD  IDX-ALPHA-FILE.
       01  ALPHA-REC.
           05 ALPHA-KEY                 PIC A(20).
           05 ALPHA-TEXT                PIC X(40).

       FD  IDX-PERF-FILE.
       01  PERF-REC.
           05 PERF-KEY                  PIC X(40).
           05 PERF-PAYLOAD              PIC X(984).

       WORKING-STORAGE SECTION.

       01  TEST-COUNTERS.
           05 TESTS-RUN                 PIC 9(5) VALUE 0.
           05 TESTS-PASSED              PIC 9(5) VALUE 0.
           05 TESTS-FAILED              PIC 9(5) VALUE 0.

       01  TEST-CONTEXT.
           05 TEST-ID                   PIC X(8)  VALUE SPACES.
           05 TEST-NAME                 PIC X(64) VALUE SPACES.
           05 ACTUAL-RESULT             PIC X(120) VALUE SPACES.
           05 EXPECTED-RESULT           PIC X(120) VALUE SPACES.

       01  FILE-STATUSES.
           05 SEQ-STATUS                PIC XX VALUE SPACES.
           05 LINESEQ-STATUS            PIC XX VALUE SPACES.
           05 IDX-STATUS                PIC XX VALUE SPACES.
           05 NUM-STATUS                PIC XX VALUE SPACES.
           05 ALPHA-STATUS              PIC XX VALUE SPACES.
           05 PERF-STATUS               PIC XX VALUE SPACES.

       01  EOF-FLAGS.
           05 SEQ-EOF                   PIC X VALUE "N".
           05 LINESEQ-EOF               PIC X VALUE "N".
           05 IDX-EOF                   PIC X VALUE "N".

       01  WORK-FIELDS.
           05 WS-INVALID-EXPECTED       PIC X(2) VALUE SPACES.

       01  PERF-FIELDS.
           05 PERF-LIMIT                PIC 9(9) VALUE 1000000.
           05 PERF-COUNTER              PIC 9(9) VALUE 0.
           05 PERF-COUNTER-EDIT         PIC ZZZ,ZZZ,ZZ9.
           05 PERF-START-TIME          PIC 9(8) VALUE 0.
           05 PERF-END-TIME            PIC 9(8) VALUE 0.
           05 PERF-START-HH            PIC 99 VALUE 0.
           05 PERF-START-MM            PIC 99 VALUE 0.
           05 PERF-START-SS            PIC 99 VALUE 0.
           05 PERF-START-HS            PIC 99 VALUE 0.
           05 PERF-END-HH              PIC 99 VALUE 0.
           05 PERF-END-MM              PIC 99 VALUE 0.
           05 PERF-END-SS              PIC 99 VALUE 0.
           05 PERF-END-HS              PIC 99 VALUE 0.
           05 PERF-TIME-WORK           PIC 9(8) VALUE 0.
           05 PERF-TIME-REMAINDER      PIC 9(8) VALUE 0.
           05 PERF-START-CENTISECONDS   PIC 9(9) VALUE 0.
           05 PERF-END-CENTISECONDS     PIC 9(9) VALUE 0.
           05 PERF-ELAPSED-CENTISECONDS PIC 9(9) VALUE 0.
           05 PERF-ELAPSED-SECONDS      PIC 9(7)V99 VALUE 0.
           05 PERF-ELAPSED-EDIT         PIC ZZZ,ZZZ,ZZ9.99.
           05 PERF-RPS                  PIC 9(9) VALUE 0.
           05 PERF-RPS-EDIT             PIC ZZZ,ZZZ,ZZ9.
           05 PERF-EST-BYTES            PIC 9(18) VALUE 0.
           05 PERF-EST-MB               PIC 9(12) VALUE 0.
           05 PERF-EST-MB-EDIT          PIC ZZZ,ZZZ,ZZZ,ZZ9.
           05 PERF-PATH                 PIC X(120)
              VALUE "/tmp/idx-perf-uuid-1m.dat".
           05 PERF-READ-COUNTER         PIC 9(9) VALUE 0.
           05 PERF-READ-FOUND           PIC 9(9) VALUE 0.
           05 PERF-READ-COUNTER-EDIT    PIC ZZZ,ZZZ,ZZ9.
           05 PERF-READ-FOUND-EDIT      PIC ZZZ,ZZZ,ZZ9.
           05 PERF-READ-START-TIME      PIC 9(8) VALUE 0.
           05 PERF-READ-END-TIME        PIC 9(8) VALUE 0.
           05 PERF-READ-START-HH        PIC 99 VALUE 0.
           05 PERF-READ-START-MM        PIC 99 VALUE 0.
           05 PERF-READ-START-SS        PIC 99 VALUE 0.
           05 PERF-READ-START-HS        PIC 99 VALUE 0.
           05 PERF-READ-END-HH          PIC 99 VALUE 0.
           05 PERF-READ-END-MM          PIC 99 VALUE 0.
           05 PERF-READ-END-SS          PIC 99 VALUE 0.
           05 PERF-READ-END-HS          PIC 99 VALUE 0.
           05 PERF-READ-START-CENTISECONDS PIC 9(9) VALUE 0.
           05 PERF-READ-END-CENTISECONDS   PIC 9(9) VALUE 0.
           05 PERF-READ-ELAPSED-CENTISECONDS PIC 9(9) VALUE 0.
           05 PERF-READ-ELAPSED-SECONDS PIC 9(7)V99 VALUE 0.
           05 PERF-READ-ELAPSED-EDIT    PIC ZZZ,ZZZ,ZZ9.99.
           05 PERF-READ-RPS             PIC 9(9) VALUE 0.
           05 PERF-READ-RPS-EDIT        PIC ZZZ,ZZZ,ZZ9.
           05 PERF-SCAN-COUNTER         PIC 9(9) VALUE 0.
           05 PERF-SCAN-FOUND           PIC 9(9) VALUE 0.
           05 PERF-SCAN-COUNTER-EDIT    PIC ZZZ,ZZZ,ZZ9.
           05 PERF-SCAN-FOUND-EDIT      PIC ZZZ,ZZZ,ZZ9.
           05 PERF-SCAN-START-TIME      PIC 9(8) VALUE 0.
           05 PERF-SCAN-END-TIME        PIC 9(8) VALUE 0.
           05 PERF-SCAN-START-HH        PIC 99 VALUE 0.
           05 PERF-SCAN-START-MM        PIC 99 VALUE 0.
           05 PERF-SCAN-START-SS        PIC 99 VALUE 0.
           05 PERF-SCAN-START-HS        PIC 99 VALUE 0.
           05 PERF-SCAN-END-HH          PIC 99 VALUE 0.
           05 PERF-SCAN-END-MM          PIC 99 VALUE 0.
           05 PERF-SCAN-END-SS          PIC 99 VALUE 0.
           05 PERF-SCAN-END-HS          PIC 99 VALUE 0.
           05 PERF-SCAN-START-CENTISECONDS PIC 9(9) VALUE 0.
           05 PERF-SCAN-END-CENTISECONDS   PIC 9(9) VALUE 0.
           05 PERF-SCAN-ELAPSED-CENTISECONDS PIC 9(9) VALUE 0.
           05 PERF-SCAN-ELAPSED-SECONDS PIC 9(7)V99 VALUE 0.
           05 PERF-SCAN-ELAPSED-EDIT    PIC ZZZ,ZZZ,ZZ9.99.
           05 PERF-SCAN-RPS             PIC 9(9) VALUE 0.
           05 PERF-SCAN-RPS-EDIT        PIC ZZZ,ZZZ,ZZ9.
           05 PERF-SCAN-EOF             PIC X VALUE "N".

       01  TIME-BREAKDOWN.
           05 TB-HH                     PIC 99 VALUE 0.
           05 TB-MM                     PIC 99 VALUE 0.
           05 TB-SS                     PIC 99 VALUE 0.
           05 TB-HS                     PIC 99 VALUE 0.

       01  UUID-BUILDER.
           05 UUID-PREFIX               PIC X(24)
              VALUE "PRCOBOL-UUID-PERF-KEY-".
           05 UUID-NUMBER               PIC 9(16) VALUE 0.

       PROCEDURE DIVISION.

       MAIN-PARA.

           DISPLAY "FILE I/O TEST SUITE".
           DISPLAY "Storage variant: 3. STORAGE IS MEMORY without compression".
           DISPLAY "===================".
           DISPLAY "All files are created under /tmp".
           DISPLAY "Existing files with the same names may be replaced.".

           PERFORM TEST-SEQUENTIAL-FILE.
           PERFORM TEST-LINE-SEQUENTIAL-FILE.
           PERFORM TEST-INDEXED-MAIN-FILE.
           PERFORM TEST-INDEXED-NUMERIC-FILE.
           PERFORM TEST-INDEXED-ALPHA-FILE.
           PERFORM PROFILE-INDEXED-UUID-1M.

           DISPLAY "===================".
           DISPLAY "TESTS RUN    : " TESTS-RUN.
           DISPLAY "TESTS PASSED : " TESTS-PASSED.
           DISPLAY "TESTS FAILED : " TESTS-FAILED.

           IF TESTS-FAILED = ZERO
               DISPLAY "RESULT       : PASS"
           ELSE
               DISPLAY "RESULT       : FAIL"
           END-IF.

           STOP RUN.

       ASSERT-RESULT.

           ADD 1 TO TESTS-RUN.

           IF ACTUAL-RESULT = EXPECTED-RESULT
               ADD 1 TO TESTS-PASSED
               DISPLAY "PASS " TEST-ID " " TEST-NAME
           ELSE
               ADD 1 TO TESTS-FAILED
               DISPLAY "FAIL " TEST-ID " " TEST-NAME
               DISPLAY "     ACTUAL   = [" ACTUAL-RESULT "]"
               DISPLAY "     EXPECTED = [" EXPECTED-RESULT "]"
           END-IF.

       ASSERT-STATUS-OK.

           MOVE "00" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

       ASSERT-STATUS-DUP.

           MOVE "22" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

       ASSERT-STATUS-INVALID.

           MOVE WS-INVALID-EXPECTED TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

       TEST-SEQUENTIAL-FILE.

           DISPLAY " ".
           DISPLAY "SEQUENTIAL FILE TESTS".
           DISPLAY "File: /tmp/seq-test.dat".

           OPEN OUTPUT SEQ-FILE.

           MOVE "S001" TO TEST-ID.
           MOVE "OPEN OUTPUT SEQUENTIAL" TO TEST-NAME.
           MOVE SEQ-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE 00001 TO SEQ-ID.
           MOVE "ALPHA" TO SEQ-NAME.
           MOVE 123.45 TO SEQ-AMOUNT.
           MOVE "FIRST" TO SEQ-FILLER.
           WRITE SEQ-REC.

           MOVE "S002" TO TEST-ID.
           MOVE "WRITE FIRST SEQUENTIAL RECORD" TO TEST-NAME.
           MOVE SEQ-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE 00002 TO SEQ-ID.
           MOVE "BETA" TO SEQ-NAME.
           MOVE -987.65 TO SEQ-AMOUNT.
           MOVE "SECOND" TO SEQ-FILLER.
           WRITE SEQ-REC.

           MOVE "S003" TO TEST-ID.
           MOVE "WRITE SECOND SEQUENTIAL RECORD" TO TEST-NAME.
           MOVE SEQ-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           CLOSE SEQ-FILE.

           OPEN INPUT SEQ-FILE.
           MOVE "S004" TO TEST-ID.
           MOVE "OPEN INPUT SEQUENTIAL" TO TEST-NAME.
           MOVE SEQ-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           READ SEQ-FILE
               AT END MOVE "Y" TO SEQ-EOF
           END-READ.

           MOVE "S005" TO TEST-ID.
           MOVE "READ FIRST SEQUENTIAL RECORD" TO TEST-NAME.
           MOVE SEQ-NAME TO ACTUAL-RESULT.
           MOVE "ALPHA" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           READ SEQ-FILE
               AT END MOVE "Y" TO SEQ-EOF
           END-READ.

           MOVE "S006" TO TEST-ID.
           MOVE "READ SECOND SEQUENTIAL RECORD" TO TEST-NAME.
           MOVE SEQ-NAME TO ACTUAL-RESULT.
           MOVE "BETA" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           READ SEQ-FILE
               AT END MOVE "Y" TO SEQ-EOF
           END-READ.

           MOVE "S007" TO TEST-ID.
           MOVE "SEQUENTIAL AT END" TO TEST-NAME.
           MOVE SEQ-EOF TO ACTUAL-RESULT.
           MOVE "Y" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           CLOSE SEQ-FILE.

       TEST-LINE-SEQUENTIAL-FILE.

           DISPLAY " ".
           DISPLAY "LINE SEQUENTIAL FILE TESTS".
           DISPLAY
           "File: /tmp/lineseq-test.txt".

           OPEN OUTPUT LINESEQ-FILE.

           MOVE "L001" TO TEST-ID.
           MOVE "OPEN OUTPUT LINE SEQUENTIAL" TO TEST-NAME.
           MOVE LINESEQ-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "FIRST LINE" TO LINESEQ-REC.
           WRITE LINESEQ-REC.
           MOVE "L002" TO TEST-ID.
           MOVE "WRITE FIRST LINE" TO TEST-NAME.
           MOVE LINESEQ-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "Second Line Mixed Case" TO LINESEQ-REC.
           WRITE LINESEQ-REC.
           MOVE "L003" TO TEST-ID.
           MOVE "WRITE MIXED CASE LINE" TO TEST-NAME.
           MOVE LINESEQ-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "lowercase line" TO LINESEQ-REC.
           WRITE LINESEQ-REC.
           MOVE "L004" TO TEST-ID.
           MOVE "WRITE LOWERCASE LINE" TO TEST-NAME.
           MOVE LINESEQ-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           CLOSE LINESEQ-FILE.

           OPEN INPUT LINESEQ-FILE.
           MOVE "L005" TO TEST-ID.
           MOVE "OPEN INPUT LINE SEQUENTIAL" TO TEST-NAME.
           MOVE LINESEQ-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           READ LINESEQ-FILE
               AT END MOVE "Y" TO LINESEQ-EOF
           END-READ.
           MOVE "L006" TO TEST-ID.
           MOVE "READ FIRST LINE" TO TEST-NAME.
           MOVE LINESEQ-REC TO ACTUAL-RESULT.
           MOVE "FIRST LINE" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           READ LINESEQ-FILE
               AT END MOVE "Y" TO LINESEQ-EOF
           END-READ.
           MOVE "L007" TO TEST-ID.
           MOVE "READ MIXED CASE LINE" TO TEST-NAME.
           MOVE LINESEQ-REC TO ACTUAL-RESULT.
           MOVE "Second Line Mixed Case" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           READ LINESEQ-FILE
               AT END MOVE "Y" TO LINESEQ-EOF
           END-READ.
           MOVE "L008" TO TEST-ID.
           MOVE "READ LOWERCASE LINE" TO TEST-NAME.
           MOVE LINESEQ-REC TO ACTUAL-RESULT.
           MOVE "lowercase line" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           READ LINESEQ-FILE
               AT END MOVE "Y" TO LINESEQ-EOF
           END-READ.
           MOVE "L009" TO TEST-ID.
           MOVE "LINE SEQUENTIAL AT END" TO TEST-NAME.
           MOVE LINESEQ-EOF TO ACTUAL-RESULT.
           MOVE "Y" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           CLOSE LINESEQ-FILE.

       TEST-INDEXED-MAIN-FILE.

           DISPLAY " ".
           DISPLAY "INDEXED FILE TESTS".
           DISPLAY "File: /tmp/idx-main.dat".

           OPEN OUTPUT IDX-MAIN-FILE.

           MOVE "I001" TO TEST-ID.
           MOVE "OPEN OUTPUT INDEXED MAIN" TO TEST-NAME.
           MOVE IDX-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "KEY-UPPER-000000000000000000000000000001" TO
                IDX-PRIMARY-KEY.
           MOVE "GROUPUPPER" TO IDX-ALT-UPPER.
           MOVE "lowerone" TO IDX-ALT-LOWER.
           MOVE "MixedOne" TO IDX-ALT-MIXED.
           MOVE "PAYLOAD UPPER KEY" TO IDX-PAYLOAD.
           WRITE IDX-REC.
           MOVE "I002" TO TEST-ID.
           MOVE "WRITE UPPERCASE PRIMARY KEY" TO TEST-NAME.
           MOVE IDX-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "key-lower-000000000000000000000000000002" TO
                IDX-PRIMARY-KEY.
           MOVE "GROUPUPPER" TO IDX-ALT-UPPER.
           MOVE "lowertwo" TO IDX-ALT-LOWER.
           MOVE "MixedTwo" TO IDX-ALT-MIXED.
           MOVE "PAYLOAD LOWER KEY" TO IDX-PAYLOAD.
           WRITE IDX-REC.
           MOVE "I003" TO TEST-ID.
           MOVE "WRITE LOWERCASE PRIMARY KEY" TO TEST-NAME.
           MOVE IDX-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "Key-Mixed-00000000000000000000000000003" TO
                IDX-PRIMARY-KEY.
           MOVE "OTHERUPPER" TO IDX-ALT-UPPER.
           MOVE "lowerthree" TO IDX-ALT-LOWER.
           MOVE "MixedTwo" TO IDX-ALT-MIXED.
           MOVE "PAYLOAD MIXED KEY" TO IDX-PAYLOAD.
           WRITE IDX-REC.
           MOVE "I004" TO TEST-ID.
           MOVE "WRITE MIXED CASE PRIMARY KEY" TO TEST-NAME.
           MOVE IDX-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "ALPHAONLYKEYABCDEFGHIJKLMNOPQRSTUVWXY" TO
                IDX-PRIMARY-KEY.
           MOVE "ALPHAUPPER" TO IDX-ALT-UPPER.
           MOVE "lowerfour" TO IDX-ALT-LOWER.
           MOVE "MixedFour" TO IDX-ALT-MIXED.
           MOVE "PAYLOAD ALPHA ONLY KEY" TO IDX-PAYLOAD.
           WRITE IDX-REC.
           MOVE "I005" TO TEST-ID.
           MOVE "WRITE ALPHABETIC-ONLY PRIMARY KEY CONTENT" TO TEST-NAME.
           MOVE IDX-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "KEY-UPPER-000000000000000000000000000001" TO
                IDX-PRIMARY-KEY.
           MOVE "DUPUPPER" TO IDX-ALT-UPPER.
           MOVE "duplower" TO IDX-ALT-LOWER.
           MOVE "DupMixed" TO IDX-ALT-MIXED.
           MOVE "DUPLICATE PRIMARY" TO IDX-PAYLOAD.
           WRITE IDX-REC
               INVALID KEY
                   CONTINUE
           END-WRITE.
           MOVE "I006" TO TEST-ID.
           MOVE "DUPLICATE PRIMARY KEY RETURNS 22" TO TEST-NAME.
           MOVE IDX-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-DUP.

           MOVE "KEY-DUPALT-0000000000000000000000000005" TO
                IDX-PRIMARY-KEY.
           MOVE "DUPUPPER" TO IDX-ALT-UPPER.
           MOVE "lowerone" TO IDX-ALT-LOWER.
           MOVE "MixedFive" TO IDX-ALT-MIXED.
           MOVE "DUPLICATE NON-DUP ALT LOWER" TO IDX-PAYLOAD.
           WRITE IDX-REC
               INVALID KEY
                   CONTINUE
           END-WRITE.
           MOVE "I007" TO TEST-ID.
           MOVE "DUPLICATE ALTERNATE KEY WITHOUT DUPLICATES" TO TEST-NAME.
           MOVE IDX-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-DUP.

           CLOSE IDX-MAIN-FILE.

           OPEN I-O IDX-MAIN-FILE.
           MOVE "I008" TO TEST-ID.
           MOVE "OPEN I-O INDEXED MAIN" TO TEST-NAME.
           MOVE IDX-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "key-lower-000000000000000000000000000002" TO
                IDX-PRIMARY-KEY.
           READ IDX-MAIN-FILE KEY IS IDX-PRIMARY-KEY
               INVALID KEY CONTINUE
           END-READ.
           MOVE "I009" TO TEST-ID.
           MOVE "RANDOM READ BY LOWERCASE PRIMARY KEY" TO TEST-NAME.
           MOVE IDX-PAYLOAD TO ACTUAL-RESULT.
           MOVE "PAYLOAD LOWER KEY" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           MOVE "lowerthree" TO IDX-ALT-LOWER.
           READ IDX-MAIN-FILE KEY IS IDX-ALT-LOWER
               INVALID KEY CONTINUE
           END-READ.
           MOVE "I010" TO TEST-ID.
           MOVE "RANDOM READ BY UNIQUE LOWER ALTERNATE KEY" TO TEST-NAME.
           MOVE IDX-PAYLOAD TO ACTUAL-RESULT.
           MOVE "PAYLOAD MIXED KEY" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           MOVE "GROUPUPPER" TO IDX-ALT-UPPER.
           START IDX-MAIN-FILE KEY IS EQUAL TO IDX-ALT-UPPER
               INVALID KEY CONTINUE
           END-START.
           READ IDX-MAIN-FILE NEXT RECORD
               AT END MOVE "Y" TO IDX-EOF
           END-READ.
           MOVE "I011" TO TEST-ID.
           MOVE "START/READ DUPLICATE UPPER ALTERNATE KEY" TO TEST-NAME.
           MOVE IDX-ALT-UPPER TO ACTUAL-RESULT.
           MOVE "GROUPUPPER" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           MOVE "MixedTwo" TO IDX-ALT-MIXED.
           START IDX-MAIN-FILE KEY IS EQUAL TO IDX-ALT-MIXED
               INVALID KEY CONTINUE
           END-START.
           READ IDX-MAIN-FILE NEXT RECORD
               AT END MOVE "Y" TO IDX-EOF
           END-READ.
           MOVE "I012" TO TEST-ID.
           MOVE "START/READ DUPLICATE MIXED ALTERNATE KEY" TO TEST-NAME.
           MOVE IDX-ALT-MIXED TO ACTUAL-RESULT.
           MOVE "MixedTwo" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           MOVE "KEY-M" TO IDX-PRIMARY-KEY.
           START IDX-MAIN-FILE KEY IS GREATER THAN OR EQUAL TO
                 IDX-PRIMARY-KEY
               INVALID KEY CONTINUE
           END-START.
           READ IDX-MAIN-FILE NEXT RECORD
               AT END MOVE "Y" TO IDX-EOF
           END-READ.
           MOVE "I013" TO TEST-ID.
           MOVE "START GE PRIMARY KEY" TO TEST-NAME.
           MOVE IDX-PRIMARY-KEY TO ACTUAL-RESULT.
           MOVE "KEY-UPPER-000000000000000000000000000001"
                TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           MOVE "ZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZZ" TO
                IDX-PRIMARY-KEY.
           START IDX-MAIN-FILE KEY IS EQUAL TO IDX-PRIMARY-KEY
               INVALID KEY
                   MOVE IDX-STATUS TO ACTUAL-RESULT
           END-START.
           MOVE "I014" TO TEST-ID.
           MOVE "START INVALID KEY RETURNS 23" TO TEST-NAME.
           MOVE "23" TO WS-INVALID-EXPECTED.
           PERFORM ASSERT-STATUS-INVALID.

           MOVE "Key-Mixed-00000000000000000000000000003" TO
                IDX-PRIMARY-KEY.
           READ IDX-MAIN-FILE KEY IS IDX-PRIMARY-KEY
               INVALID KEY CONTINUE
           END-READ.
           MOVE "UPDATED MIXED KEY PAYLOAD" TO IDX-PAYLOAD.
           REWRITE IDX-REC
               INVALID KEY CONTINUE
           END-REWRITE.
           MOVE "I015" TO TEST-ID.
           MOVE "REWRITE INDEXED RECORD" TO TEST-NAME.
           MOVE IDX-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "Key-Mixed-00000000000000000000000000003" TO
                IDX-PRIMARY-KEY.
           READ IDX-MAIN-FILE KEY IS IDX-PRIMARY-KEY
               INVALID KEY CONTINUE
           END-READ.
           MOVE "I016" TO TEST-ID.
           MOVE "READ REWRITTEN INDEXED RECORD" TO TEST-NAME.
           MOVE IDX-PAYLOAD TO ACTUAL-RESULT.
           MOVE "UPDATED MIXED KEY PAYLOAD" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           DELETE IDX-MAIN-FILE RECORD
               INVALID KEY CONTINUE
           END-DELETE.
           MOVE "I017" TO TEST-ID.
           MOVE "DELETE CURRENT INDEXED RECORD" TO TEST-NAME.
           MOVE IDX-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "Key-Mixed-00000000000000000000000000003" TO
                IDX-PRIMARY-KEY.
           READ IDX-MAIN-FILE KEY IS IDX-PRIMARY-KEY
               INVALID KEY
                   MOVE IDX-STATUS TO ACTUAL-RESULT
           END-READ.
           MOVE "I018" TO TEST-ID.
           MOVE "READ DELETED RECORD RETURNS 23" TO TEST-NAME.
           MOVE "23" TO WS-INVALID-EXPECTED.
           PERFORM ASSERT-STATUS-INVALID.

           CLOSE IDX-MAIN-FILE.

       TEST-INDEXED-NUMERIC-FILE.

           DISPLAY " ".
           DISPLAY "INDEXED NUMERIC KEY TESTS".
           DISPLAY "File: /tmp/idx-numeric.dat".

           OPEN OUTPUT IDX-NUM-FILE.
           MOVE "N001" TO TEST-ID.
           MOVE "OPEN OUTPUT NUMERIC KEY INDEXED FILE" TO TEST-NAME.
           MOVE NUM-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE 00000001 TO NUM-KEY.
           MOVE "NUMERIC KEY ONE" TO NUM-TEXT.
           WRITE NUM-REC.
           MOVE "N002" TO TEST-ID.
           MOVE "WRITE NUMERIC KEY 00000001" TO TEST-NAME.
           MOVE NUM-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE 00001000 TO NUM-KEY.
           MOVE "NUMERIC KEY ONE THOUSAND" TO NUM-TEXT.
           WRITE NUM-REC.
           MOVE "N003" TO TEST-ID.
           MOVE "WRITE NUMERIC KEY 00001000" TO TEST-NAME.
           MOVE NUM-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           CLOSE IDX-NUM-FILE.

           OPEN INPUT IDX-NUM-FILE.
           MOVE 00001000 TO NUM-KEY.
           READ IDX-NUM-FILE KEY IS NUM-KEY
               INVALID KEY CONTINUE
           END-READ.
           MOVE "N004" TO TEST-ID.
           MOVE "RANDOM READ NUMERIC DISPLAY KEY" TO TEST-NAME.
           MOVE NUM-TEXT TO ACTUAL-RESULT.
           MOVE "NUMERIC KEY ONE THOUSAND" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           CLOSE IDX-NUM-FILE.

       TEST-INDEXED-ALPHA-FILE.

           DISPLAY " ".
           DISPLAY "INDEXED ALPHABETIC KEY TESTS".
           DISPLAY "File: /tmp/idx-alpha.dat".

           OPEN OUTPUT IDX-ALPHA-FILE.
           MOVE "A001" TO TEST-ID.
           MOVE "OPEN OUTPUT ALPHA KEY INDEXED FILE" TO TEST-NAME.
           MOVE ALPHA-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "ALPHAKEYONE" TO ALPHA-KEY.
           MOVE "UPPER ALPHA KEY" TO ALPHA-TEXT.
           WRITE ALPHA-REC.
           MOVE "A002" TO TEST-ID.
           MOVE "WRITE UPPER ALPHA KEY" TO TEST-NAME.
           MOVE ALPHA-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "lowerkeytwo" TO ALPHA-KEY.
           MOVE "LOWER ALPHA KEY" TO ALPHA-TEXT.
           WRITE ALPHA-REC.
           MOVE "A003" TO TEST-ID.
           MOVE "WRITE LOWER ALPHA KEY CONTENT" TO TEST-NAME.
           MOVE ALPHA-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE "MixedKeyThree" TO ALPHA-KEY.
           MOVE "MIXED ALPHA KEY" TO ALPHA-TEXT.
           WRITE ALPHA-REC.
           MOVE "A004" TO TEST-ID.
           MOVE "WRITE MIXED ALPHA KEY CONTENT" TO TEST-NAME.
           MOVE ALPHA-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           CLOSE IDX-ALPHA-FILE.

           OPEN INPUT IDX-ALPHA-FILE.
           MOVE "MixedKeyThree" TO ALPHA-KEY.
           READ IDX-ALPHA-FILE KEY IS ALPHA-KEY
               INVALID KEY CONTINUE
           END-READ.
           MOVE "A005" TO TEST-ID.
           MOVE "RANDOM READ MIXED ALPHA KEY" TO TEST-NAME.
           MOVE ALPHA-TEXT TO ACTUAL-RESULT.
           MOVE "MIXED ALPHA KEY" TO EXPECTED-RESULT.
           PERFORM ASSERT-RESULT.

           CLOSE IDX-ALPHA-FILE.

       PROFILE-INDEXED-UUID-1M.

           DISPLAY " ".
           DISPLAY "INDEXED PERFORMANCE PROFILE".
           DISPLAY "File kept on disk after test.".
           DISPLAY "File path: " PERF-PATH.
           DISPLAY "Record count target: 1,000,000".
           DISPLAY "Primary key size: 40 bytes".
           DISPLAY "Record size: 1024 bytes".

           ACCEPT PERF-START-TIME FROM TIME.
           PERFORM CONVERT-START-TIME.

           OPEN OUTPUT IDX-PERF-FILE.
           MOVE "P001" TO TEST-ID.
           MOVE "OPEN OUTPUT PERFORMANCE INDEXED FILE" TO TEST-NAME.
           MOVE PERF-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE ALL "X" TO PERF-PAYLOAD.
           MOVE ZERO TO PERF-COUNTER.

           PERFORM UNTIL PERF-COUNTER >= PERF-LIMIT
               ADD 1 TO PERF-COUNTER
               MOVE PERF-COUNTER TO UUID-NUMBER
               MOVE SPACES TO PERF-KEY
               STRING UUID-PREFIX DELIMITED BY SIZE
                      UUID-NUMBER DELIMITED BY SIZE
                      INTO PERF-KEY
               WRITE PERF-REC
                   INVALID KEY
                       DISPLAY "PERFORMANCE WRITE FAILED AT RECORD "
                               PERF-COUNTER
                       DISPLAY "FILE STATUS: " PERF-STATUS
                       MOVE PERF-LIMIT TO PERF-COUNTER
               END-WRITE
           END-PERFORM.

           CLOSE IDX-PERF-FILE.

           ACCEPT PERF-END-TIME FROM TIME.
           PERFORM CONVERT-END-TIME.
           PERFORM CALCULATE-PERF-STATS.

           MOVE "P002" TO TEST-ID.
           MOVE "PERFORMANCE FILE WRITE LOOP COMPLETED" TO TEST-NAME.
           MOVE PERF-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE PERF-COUNTER TO PERF-COUNTER-EDIT.
           MOVE PERF-ELAPSED-SECONDS TO PERF-ELAPSED-EDIT.
           MOVE PERF-RPS TO PERF-RPS-EDIT.
           MOVE PERF-EST-MB TO PERF-EST-MB-EDIT.

           DISPLAY " ".
           DISPLAY "INDEXED WRITE PERFORMANCE STATISTICS".
           DISPLAY "Path                : " PERF-PATH.
           DISPLAY "Start time HHMMSSHS : " PERF-START-TIME.
           DISPLAY "End time HHMMSSHS   : " PERF-END-TIME.
           DISPLAY "Records attempted   : " PERF-COUNTER-EDIT.
           DISPLAY "Elapsed seconds     : " PERF-ELAPSED-EDIT.
           DISPLAY "Records per second  : " PERF-RPS-EDIT.
           DISPLAY "Approx data MB      : " PERF-EST-MB-EDIT.
           DISPLAY "Final file status   : " PERF-STATUS.

           ACCEPT PERF-READ-START-TIME FROM TIME.
           PERFORM CONVERT-READ-START-TIME.

           OPEN INPUT IDX-PERF-FILE.
           MOVE "P003" TO TEST-ID.
           MOVE "OPEN INPUT PERFORMANCE INDEXED FILE" TO TEST-NAME.
           MOVE PERF-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE ZERO TO PERF-READ-COUNTER.
           MOVE ZERO TO PERF-READ-FOUND.

           PERFORM UNTIL PERF-READ-COUNTER >= PERF-LIMIT
               ADD 1 TO PERF-READ-COUNTER
               MOVE PERF-READ-COUNTER TO UUID-NUMBER
               MOVE SPACES TO PERF-KEY
               STRING UUID-PREFIX DELIMITED BY SIZE
                      UUID-NUMBER DELIMITED BY SIZE
                      INTO PERF-KEY
               READ IDX-PERF-FILE KEY IS PERF-KEY
                   INVALID KEY
                       CONTINUE
               END-READ
               IF PERF-STATUS = "00"
                   ADD 1 TO PERF-READ-FOUND
               ELSE
                   DISPLAY "PERFORMANCE READ FAILED AT RECORD "
                           PERF-READ-COUNTER
                   DISPLAY "FILE STATUS: " PERF-STATUS
                   MOVE PERF-LIMIT TO PERF-READ-COUNTER
               END-IF
           END-PERFORM.

           CLOSE IDX-PERF-FILE.

           ACCEPT PERF-READ-END-TIME FROM TIME.
           PERFORM CONVERT-READ-END-TIME.
           PERFORM CALCULATE-READ-PERF-STATS.

           MOVE "P004" TO TEST-ID.
           MOVE "PERFORMANCE FILE READ LOOP COMPLETED" TO TEST-NAME.
           MOVE PERF-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE PERF-READ-COUNTER TO PERF-READ-COUNTER-EDIT.
           MOVE PERF-READ-FOUND TO PERF-READ-FOUND-EDIT.
           MOVE PERF-READ-ELAPSED-SECONDS TO PERF-READ-ELAPSED-EDIT.
           MOVE PERF-READ-RPS TO PERF-READ-RPS-EDIT.

           DISPLAY " ".
           DISPLAY "INDEXED READ PERFORMANCE STATISTICS".
           DISPLAY "Path                : " PERF-PATH.
           DISPLAY "Start time HHMMSSHS : " PERF-READ-START-TIME.
           DISPLAY "End time HHMMSSHS   : " PERF-READ-END-TIME.
           DISPLAY "Records attempted   : " PERF-READ-COUNTER-EDIT.
           DISPLAY "Records found       : " PERF-READ-FOUND-EDIT.
           DISPLAY "Elapsed seconds     : " PERF-READ-ELAPSED-EDIT.
           DISPLAY "Records per second  : " PERF-READ-RPS-EDIT.
           DISPLAY "Final file status   : " PERF-STATUS.

           ACCEPT PERF-SCAN-START-TIME FROM TIME.
           PERFORM CONVERT-SCAN-START-TIME.

           OPEN INPUT IDX-PERF-FILE.
           MOVE "P005" TO TEST-ID.
           MOVE "OPEN INPUT PERFORMANCE INDEXED FILE FOR SCAN"
                TO TEST-NAME.
           MOVE PERF-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE ZERO TO PERF-SCAN-COUNTER.
           MOVE ZERO TO PERF-SCAN-FOUND.
           MOVE "N" TO PERF-SCAN-EOF.

           PERFORM UNTIL PERF-SCAN-EOF = "Y"
               READ IDX-PERF-FILE NEXT RECORD
                   AT END
                       MOVE "Y" TO PERF-SCAN-EOF
                   NOT AT END
                       ADD 1 TO PERF-SCAN-COUNTER
                       ADD 1 TO PERF-SCAN-FOUND
               END-READ
           END-PERFORM.

           CLOSE IDX-PERF-FILE.

           ACCEPT PERF-SCAN-END-TIME FROM TIME.
           PERFORM CONVERT-SCAN-END-TIME.
           PERFORM CALCULATE-SCAN-PERF-STATS.

           MOVE "P006" TO TEST-ID.
           MOVE "PERFORMANCE FILE SEQUENTIAL SCAN COMPLETED"
                TO TEST-NAME.
           MOVE PERF-STATUS TO ACTUAL-RESULT.
           PERFORM ASSERT-STATUS-OK.

           MOVE PERF-SCAN-COUNTER TO PERF-SCAN-COUNTER-EDIT.
           MOVE PERF-SCAN-FOUND TO PERF-SCAN-FOUND-EDIT.
           MOVE PERF-SCAN-ELAPSED-SECONDS TO PERF-SCAN-ELAPSED-EDIT.
           MOVE PERF-SCAN-RPS TO PERF-SCAN-RPS-EDIT.

           DISPLAY " ".
           DISPLAY "INDEXED SCAN PERFORMANCE STATISTICS".
           DISPLAY "Path                : " PERF-PATH.
           DISPLAY "Start time HHMMSSHS : " PERF-SCAN-START-TIME.
           DISPLAY "End time HHMMSSHS   : " PERF-SCAN-END-TIME.
           DISPLAY "Records scanned     : " PERF-SCAN-COUNTER-EDIT.
           DISPLAY "Records found       : " PERF-SCAN-FOUND-EDIT.
           DISPLAY "Elapsed seconds     : " PERF-SCAN-ELAPSED-EDIT.
           DISPLAY "Records per second  : " PERF-SCAN-RPS-EDIT.
           DISPLAY "Final file status   : " PERF-STATUS.
           DISPLAY "The indexed performance file was intentionally kept.".

       CONVERT-START-TIME.

           MOVE PERF-START-TIME TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 1000000
               GIVING PERF-START-HH
               REMAINDER PERF-TIME-REMAINDER.
           MOVE PERF-TIME-REMAINDER TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 10000
               GIVING PERF-START-MM
               REMAINDER PERF-TIME-REMAINDER.
           MOVE PERF-TIME-REMAINDER TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 100
               GIVING PERF-START-SS
               REMAINDER PERF-START-HS.

           COMPUTE PERF-START-CENTISECONDS =
               (((PERF-START-HH * 60 + PERF-START-MM) * 60
                 + PERF-START-SS) * 100)
               + PERF-START-HS.

       CONVERT-END-TIME.

           MOVE PERF-END-TIME TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 1000000
               GIVING PERF-END-HH
               REMAINDER PERF-TIME-REMAINDER.
           MOVE PERF-TIME-REMAINDER TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 10000
               GIVING PERF-END-MM
               REMAINDER PERF-TIME-REMAINDER.
           MOVE PERF-TIME-REMAINDER TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 100
               GIVING PERF-END-SS
               REMAINDER PERF-END-HS.

           COMPUTE PERF-END-CENTISECONDS =
               (((PERF-END-HH * 60 + PERF-END-MM) * 60
                 + PERF-END-SS) * 100)
               + PERF-END-HS.

       CALCULATE-PERF-STATS.

           IF PERF-END-CENTISECONDS < PERF-START-CENTISECONDS
               ADD 8640000 TO PERF-END-CENTISECONDS
           END-IF.

           COMPUTE PERF-ELAPSED-CENTISECONDS =
               PERF-END-CENTISECONDS - PERF-START-CENTISECONDS.

           IF PERF-ELAPSED-CENTISECONDS = ZERO
               MOVE 1 TO PERF-ELAPSED-CENTISECONDS
           END-IF.

           COMPUTE PERF-ELAPSED-SECONDS =
               PERF-ELAPSED-CENTISECONDS / 100.

           COMPUTE PERF-RPS =
               (PERF-COUNTER * 100) / PERF-ELAPSED-CENTISECONDS.

           COMPUTE PERF-EST-BYTES =
               PERF-COUNTER * 1024.

           COMPUTE PERF-EST-MB =
               PERF-EST-BYTES / 1048576.

       CONVERT-READ-START-TIME.

           MOVE PERF-READ-START-TIME TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 1000000
               GIVING PERF-READ-START-HH
               REMAINDER PERF-TIME-REMAINDER.
           MOVE PERF-TIME-REMAINDER TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 10000
               GIVING PERF-READ-START-MM
               REMAINDER PERF-TIME-REMAINDER.
           MOVE PERF-TIME-REMAINDER TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 100
               GIVING PERF-READ-START-SS
               REMAINDER PERF-READ-START-HS.

           COMPUTE PERF-READ-START-CENTISECONDS =
               (((PERF-READ-START-HH * 60 + PERF-READ-START-MM) * 60
                 + PERF-READ-START-SS) * 100)
               + PERF-READ-START-HS.

       CONVERT-READ-END-TIME.

           MOVE PERF-READ-END-TIME TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 1000000
               GIVING PERF-READ-END-HH
               REMAINDER PERF-TIME-REMAINDER.
           MOVE PERF-TIME-REMAINDER TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 10000
               GIVING PERF-READ-END-MM
               REMAINDER PERF-TIME-REMAINDER.
           MOVE PERF-TIME-REMAINDER TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 100
               GIVING PERF-READ-END-SS
               REMAINDER PERF-READ-END-HS.

           COMPUTE PERF-READ-END-CENTISECONDS =
               (((PERF-READ-END-HH * 60 + PERF-READ-END-MM) * 60
                 + PERF-READ-END-SS) * 100)
               + PERF-READ-END-HS.

       CALCULATE-READ-PERF-STATS.

           IF PERF-READ-END-CENTISECONDS < PERF-READ-START-CENTISECONDS
               ADD 8640000 TO PERF-READ-END-CENTISECONDS
           END-IF.

           COMPUTE PERF-READ-ELAPSED-CENTISECONDS =
               PERF-READ-END-CENTISECONDS
               - PERF-READ-START-CENTISECONDS.

           IF PERF-READ-ELAPSED-CENTISECONDS = ZERO
               MOVE 1 TO PERF-READ-ELAPSED-CENTISECONDS
           END-IF.

           COMPUTE PERF-READ-ELAPSED-SECONDS =
               PERF-READ-ELAPSED-CENTISECONDS / 100.

           COMPUTE PERF-READ-RPS =
               (PERF-READ-FOUND * 100)
               / PERF-READ-ELAPSED-CENTISECONDS.

       CONVERT-SCAN-START-TIME.

           MOVE PERF-SCAN-START-TIME TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 1000000
               GIVING PERF-SCAN-START-HH
               REMAINDER PERF-TIME-REMAINDER.
           MOVE PERF-TIME-REMAINDER TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 10000
               GIVING PERF-SCAN-START-MM
               REMAINDER PERF-TIME-REMAINDER.
           MOVE PERF-TIME-REMAINDER TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 100
               GIVING PERF-SCAN-START-SS
               REMAINDER PERF-SCAN-START-HS.

           COMPUTE PERF-SCAN-START-CENTISECONDS =
               (((PERF-SCAN-START-HH * 60 + PERF-SCAN-START-MM) * 60
                 + PERF-SCAN-START-SS) * 100)
               + PERF-SCAN-START-HS.

       CONVERT-SCAN-END-TIME.

           MOVE PERF-SCAN-END-TIME TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 1000000
               GIVING PERF-SCAN-END-HH
               REMAINDER PERF-TIME-REMAINDER.
           MOVE PERF-TIME-REMAINDER TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 10000
               GIVING PERF-SCAN-END-MM
               REMAINDER PERF-TIME-REMAINDER.
           MOVE PERF-TIME-REMAINDER TO PERF-TIME-WORK.
           DIVIDE PERF-TIME-WORK BY 100
               GIVING PERF-SCAN-END-SS
               REMAINDER PERF-SCAN-END-HS.

           COMPUTE PERF-SCAN-END-CENTISECONDS =
               (((PERF-SCAN-END-HH * 60 + PERF-SCAN-END-MM) * 60
                 + PERF-SCAN-END-SS) * 100)
               + PERF-SCAN-END-HS.

       CALCULATE-SCAN-PERF-STATS.

           IF PERF-SCAN-END-CENTISECONDS < PERF-SCAN-START-CENTISECONDS
               ADD 8640000 TO PERF-SCAN-END-CENTISECONDS
           END-IF.

           COMPUTE PERF-SCAN-ELAPSED-CENTISECONDS =
               PERF-SCAN-END-CENTISECONDS
               - PERF-SCAN-START-CENTISECONDS.

           IF PERF-SCAN-ELAPSED-CENTISECONDS = ZERO
               MOVE 1 TO PERF-SCAN-ELAPSED-CENTISECONDS
           END-IF.

           COMPUTE PERF-SCAN-ELAPSED-SECONDS =
               PERF-SCAN-ELAPSED-CENTISECONDS / 100.

           COMPUTE PERF-SCAN-RPS =
               (PERF-SCAN-FOUND * 100)
               / PERF-SCAN-ELAPSED-CENTISECONDS.
