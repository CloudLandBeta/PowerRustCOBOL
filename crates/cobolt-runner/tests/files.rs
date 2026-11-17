// SPDX-License-Identifier: Apache-2.0
// Copyright (c) 2026 Emerson Lopes and PowerRustCOBOL contributors
//
// Licensed under the Apache License, Version 2.0.
// See the LICENSE file in the project root for full license information.

//! DESKTOP-WASM-001 T38 (R11 for files, AC9 groundwork): a desktop program's files on disk, of
//! every organisation, behave on the WebAssembly path as they do under the interpreter. Each
//! program below runs twice, packaged into a copy of the built `prc-runner` and under `rcrun run`,
//! each in a folder of its own. The two runs must print the same lines (every FILE STATUS is
//! printed) and leave the same files, byte for byte.

use std::collections::BTreeMap;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};
use std::time::Instant;

use cobolt_program_package::Kind;

/// SEQUENTIAL and LINE SEQUENTIAL: OUTPUT, EXTEND, I-O with REWRITE, INPUT to the end and past it,
/// an OPEN of an open file and a CLOSE of a closed one, CLOSE WITH LOCK, an absent file with and
/// without OPTIONAL, a path held in a data item, and a variable-length file with DEPENDING ON.
const SEQUENTIAL: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T38SEQ.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT SEQ-FILE ASSIGN TO \"seq.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT LINE-FILE ASSIGN TO WS-LINE-PATH
        ORGANIZATION IS LINE SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT OPTIONAL OPT-FILE ASSIGN TO \"optional.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT MISSING-FILE ASSIGN TO \"missing.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT VAR-FILE ASSIGN TO \"var.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD SEQ-FILE.
01 SEQ-REC.
   05 SEQ-ID   PIC 9(4).
   05 SEQ-NAME PIC X(16).
FD LINE-FILE.
01 LINE-REC PIC X(30).
FD OPT-FILE.
01 OPT-REC PIC X(10).
FD MISSING-FILE.
01 MISSING-REC PIC X(10).
FD VAR-FILE
   RECORD VARYING FROM 5 TO 40 CHARACTERS DEPENDING ON WS-LEN.
01 VAR-REC PIC X(40).
WORKING-STORAGE SECTION.
01 WS-FS        PIC XX.
01 WS-LINE-PATH PIC X(40) VALUE \"lines.txt\".
01 WS-LEN       PIC 99.
01 WS-I         PIC 99.
01 WS-EOF       PIC X.
PROCEDURE DIVISION.
    OPEN OUTPUT SEQ-FILE
    DISPLAY \"open output \" WS-FS
    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 5
        MOVE WS-I TO SEQ-ID
        MOVE \"RECORD\" TO SEQ-NAME
        WRITE SEQ-REC
        DISPLAY \"write \" WS-I \" \" WS-FS
    END-PERFORM
    OPEN OUTPUT SEQ-FILE
    DISPLAY \"open of an open file \" WS-FS
    CLOSE SEQ-FILE
    DISPLAY \"close \" WS-FS
    CLOSE SEQ-FILE
    DISPLAY \"close of a closed file \" WS-FS
    OPEN EXTEND SEQ-FILE
    MOVE 6 TO SEQ-ID
    MOVE \"EXTENDED\" TO SEQ-NAME
    WRITE SEQ-REC
    DISPLAY \"extend write \" WS-FS
    CLOSE SEQ-FILE
    OPEN I-O SEQ-FILE
    READ SEQ-FILE
    DISPLAY \"read \" SEQ-ID \" \" SEQ-NAME \" \" WS-FS
    MOVE \"REWRITTEN\" TO SEQ-NAME
    REWRITE SEQ-REC
    DISPLAY \"rewrite \" WS-FS
    REWRITE SEQ-REC
    DISPLAY \"rewrite with no read \" WS-FS
    CLOSE SEQ-FILE
    OPEN INPUT SEQ-FILE
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ SEQ-FILE
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"seq \" SEQ-ID \" \" SEQ-NAME
        END-READ
    END-PERFORM
    DISPLAY \"at end \" WS-FS
    READ SEQ-FILE
        AT END DISPLAY \"at end again\"
    END-READ
    DISPLAY \"past the end \" WS-FS
    CLOSE SEQ-FILE WITH LOCK
    DISPLAY \"close with lock \" WS-FS
    OPEN INPUT SEQ-FILE
    DISPLAY \"open after lock \" WS-FS
    OPEN OUTPUT LINE-FILE
    MOVE \"first line\" TO LINE-REC
    WRITE LINE-REC
    MOVE \"  second, indented\" TO LINE-REC
    WRITE LINE-REC
    CLOSE LINE-FILE
    OPEN INPUT LINE-FILE
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ LINE-FILE
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"line [\" LINE-REC \"]\"
        END-READ
    END-PERFORM
    CLOSE LINE-FILE
    OPEN INPUT MISSING-FILE
    DISPLAY \"absent \" WS-FS
    OPEN INPUT OPT-FILE
    DISPLAY \"absent optional \" WS-FS
    READ OPT-FILE
        AT END DISPLAY \"optional at end \" WS-FS
    END-READ
    CLOSE OPT-FILE
    OPEN OUTPUT VAR-FILE
    MOVE 10 TO WS-LEN
    MOVE \"ten bytes!\" TO VAR-REC
    WRITE VAR-REC
    DISPLAY \"var write 10 \" WS-FS
    MOVE 3 TO WS-LEN
    WRITE VAR-REC
    DISPLAY \"var write 3 \" WS-FS
    MOVE 25 TO WS-LEN
    MOVE \"twenty-five bytes long...\" TO VAR-REC
    WRITE VAR-REC
    DISPLAY \"var write 25 \" WS-FS
    CLOSE VAR-FILE
    OPEN INPUT VAR-FILE
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ VAR-FILE
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"var \" WS-LEN \" [\" VAR-REC \"]\"
        END-READ
    END-PERFORM
    CLOSE VAR-FILE
    STOP RUN.
";

/// RELATIVE: a sequential creator told each record's number (and status 14 once the number no
/// longer fits its one-digit RELATIVE KEY), then dynamic access: WRITE by number, a duplicate
/// (22), READ by number and a missing one (23), REWRITE, DELETE, START and READ NEXT.
const RELATIVE: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T38REL.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT REL-SEQ ASSIGN TO \"relseq.dat\"
        ORGANIZATION IS RELATIVE
        ACCESS MODE IS SEQUENTIAL
        RELATIVE KEY IS WS-SK
        FILE STATUS IS WS-FS.
    SELECT REL-FILE ASSIGN TO \"rel.dat\"
        ORGANIZATION IS RELATIVE
        ACCESS MODE IS DYNAMIC
        RELATIVE KEY IS WS-RK
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD REL-SEQ.
01 SEQ-REC.
   05 SEQ-TAG PIC X(4).
   05 SEQ-NUM PIC 99.
   05 FILLER  PIC XX.
FD REL-FILE.
01 REL-REC.
   05 REL-ID   PIC 9(3).
   05 REL-TEXT PIC X(12).
WORKING-STORAGE SECTION.
01 WS-FS  PIC XX.
01 WS-SK  PIC 9.
01 WS-RK  PIC 9(4).
01 WS-I   PIC 99.
01 WS-EOF PIC X.
PROCEDURE DIVISION.
    OPEN OUTPUT REL-SEQ
    DISPLAY \"open output \" WS-FS
    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 11
        MOVE SPACES TO SEQ-REC
        MOVE \"ROW\" TO SEQ-TAG
        MOVE WS-I TO SEQ-NUM
        WRITE SEQ-REC
        DISPLAY \"write \" WS-I \" key \" WS-SK \" \" WS-FS
    END-PERFORM
    CLOSE REL-SEQ
    OPEN INPUT REL-SEQ
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ REL-SEQ
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"read key \" WS-SK \" [\" SEQ-REC \"]\"
        END-READ
    END-PERFORM
    DISPLAY \"end \" WS-FS
    CLOSE REL-SEQ
    OPEN OUTPUT REL-FILE
    MOVE 5 TO WS-RK
    MOVE 105 TO REL-ID
    MOVE \"FIVE\" TO REL-TEXT
    WRITE REL-REC
        INVALID KEY DISPLAY \"invalid\"
    END-WRITE
    DISPLAY \"write 5 \" WS-FS
    MOVE 2 TO WS-RK
    MOVE 102 TO REL-ID
    MOVE \"TWO\" TO REL-TEXT
    WRITE REL-REC
    DISPLAY \"write 2 \" WS-FS
    MOVE 9 TO WS-RK
    MOVE 109 TO REL-ID
    MOVE \"NINE\" TO REL-TEXT
    WRITE REL-REC
    DISPLAY \"write 9 \" WS-FS
    MOVE 5 TO WS-RK
    WRITE REL-REC
        INVALID KEY DISPLAY \"write 5 again invalid \" WS-FS
    END-WRITE
    CLOSE REL-FILE
    OPEN I-O REL-FILE
    MOVE 2 TO WS-RK
    READ REL-FILE
        INVALID KEY DISPLAY \"read 2 invalid\"
    END-READ
    DISPLAY \"read 2 \" REL-ID \" \" REL-TEXT \" \" WS-FS
    MOVE 3 TO WS-RK
    READ REL-FILE
        INVALID KEY DISPLAY \"read 3 invalid \" WS-FS
    END-READ
    MOVE 2 TO WS-RK
    MOVE \"TWO, AGAIN\" TO REL-TEXT
    REWRITE REL-REC
    DISPLAY \"rewrite 2 \" WS-FS
    MOVE 9 TO WS-RK
    DELETE REL-FILE
    DISPLAY \"delete 9 \" WS-FS
    READ REL-FILE
        INVALID KEY DISPLAY \"read 9 invalid \" WS-FS
    END-READ
    MOVE 1 TO WS-RK
    START REL-FILE KEY IS NOT LESS THAN WS-RK
    DISPLAY \"start \" WS-FS
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ REL-FILE NEXT RECORD
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"next \" WS-RK \" \" REL-ID \" \" REL-TEXT
        END-READ
    END-PERFORM
    CLOSE REL-FILE
    STOP RUN.
";

/// INDEXED: dynamic access with an alternate key WITH DUPLICATES (records written out of key
/// order, a duplicate primary key 22, READ by key and a missing one 23, REWRITE, DELETE, START on
/// the primary and on the alternate key, READ NEXT), then sequential access, where a key out of
/// order is 21.
const INDEXED: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T38IDX.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT CUST-FILE ASSIGN TO \"cust.idx\"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS DYNAMIC
        RECORD KEY IS CUST-ID
        ALTERNATE RECORD KEY IS CUST-CITY WITH DUPLICATES
        FILE STATUS IS WS-FS.
    SELECT SEQ-IDX ASSIGN TO \"seq.idx\"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS SEQUENTIAL
        RECORD KEY IS SQ-KEY
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD CUST-FILE.
01 CUST-REC.
   05 CUST-ID   PIC 9(5).
   05 CUST-NAME PIC X(15).
   05 CUST-CITY PIC X(10).
FD SEQ-IDX.
01 SQ-REC.
   05 SQ-KEY  PIC 9(3).
   05 SQ-DATA PIC X(7).
WORKING-STORAGE SECTION.
01 WS-FS  PIC XX.
01 WS-EOF PIC X.
PROCEDURE DIVISION.
    OPEN OUTPUT CUST-FILE
    DISPLAY \"open output \" WS-FS
    MOVE 30 TO CUST-ID
    MOVE \"CARLA\" TO CUST-NAME
    MOVE \"RIO\" TO CUST-CITY
    WRITE CUST-REC
    DISPLAY \"write 30 \" WS-FS
    MOVE 10 TO CUST-ID
    MOVE \"ANA\" TO CUST-NAME
    MOVE \"SAO PAULO\" TO CUST-CITY
    WRITE CUST-REC
    DISPLAY \"write 10 \" WS-FS
    MOVE 50 TO CUST-ID
    MOVE \"EVA\" TO CUST-NAME
    MOVE \"RIO\" TO CUST-CITY
    WRITE CUST-REC
    DISPLAY \"write 50 \" WS-FS
    MOVE 20 TO CUST-ID
    MOVE \"BRUNO\" TO CUST-NAME
    MOVE \"RECIFE\" TO CUST-CITY
    WRITE CUST-REC
    DISPLAY \"write 20 \" WS-FS
    MOVE 40 TO CUST-ID
    MOVE \"DAVI\" TO CUST-NAME
    MOVE \"RIO\" TO CUST-CITY
    WRITE CUST-REC
    DISPLAY \"write 40 \" WS-FS
    MOVE 10 TO CUST-ID
    WRITE CUST-REC
        INVALID KEY DISPLAY \"write 10 again invalid \" WS-FS
    END-WRITE
    CLOSE CUST-FILE
    OPEN I-O CUST-FILE
    MOVE 20 TO CUST-ID
    READ CUST-FILE
        INVALID KEY DISPLAY \"read 20 invalid\"
    END-READ
    DISPLAY \"read 20 \" CUST-NAME \" \" CUST-CITY \" \" WS-FS
    MOVE 99 TO CUST-ID
    READ CUST-FILE
        INVALID KEY DISPLAY \"read 99 invalid \" WS-FS
    END-READ
    MOVE 20 TO CUST-ID
    MOVE \"BRUNO SILVA\" TO CUST-NAME
    MOVE \"RECIFE\" TO CUST-CITY
    REWRITE CUST-REC
    DISPLAY \"rewrite 20 \" WS-FS
    MOVE 50 TO CUST-ID
    DELETE CUST-FILE
    DISPLAY \"delete 50 \" WS-FS
    MOVE 15 TO CUST-ID
    START CUST-FILE KEY IS NOT LESS THAN CUST-ID
    DISPLAY \"start id 15 \" WS-FS
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ CUST-FILE NEXT RECORD
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"by id \" CUST-ID \" \" CUST-NAME \" \" CUST-CITY
        END-READ
    END-PERFORM
    MOVE \"RIO\" TO CUST-CITY
    START CUST-FILE KEY IS EQUAL TO CUST-CITY
    DISPLAY \"start city RIO \" WS-FS
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ CUST-FILE NEXT RECORD
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"by city \" CUST-CITY \" \" CUST-ID \" \" CUST-NAME
        END-READ
    END-PERFORM
    CLOSE CUST-FILE
    OPEN OUTPUT SEQ-IDX
    MOVE 1 TO SQ-KEY
    MOVE \"ONE\" TO SQ-DATA
    WRITE SQ-REC
    DISPLAY \"seq write 1 \" WS-FS
    MOVE 2 TO SQ-KEY
    MOVE \"TWO\" TO SQ-DATA
    WRITE SQ-REC
    DISPLAY \"seq write 2 \" WS-FS
    MOVE 5 TO SQ-KEY
    MOVE \"FIVE\" TO SQ-DATA
    WRITE SQ-REC
    DISPLAY \"seq write 5 \" WS-FS
    MOVE 4 TO SQ-KEY
    MOVE \"FOUR\" TO SQ-DATA
    WRITE SQ-REC
        INVALID KEY DISPLAY \"seq write 4 out of order \" WS-FS
    END-WRITE
    CLOSE SEQ-IDX
    OPEN INPUT SEQ-IDX
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ SEQ-IDX
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"seq \" SQ-KEY \" \" SQ-DATA
        END-READ
    END-PERFORM
    CLOSE SEQ-IDX
    STOP RUN.
";

/// SORT and MERGE with RELATIVE and INDEXED files (T39c): an indexed USING file read in key order,
/// a relative one read past its empty slots, a relative GIVING file written to slots 1, 2, 3, …
/// with its RELATIVE KEY left on the last, an indexed GIVING file in the sequential access mode,
/// a MERGE of two indexed files, and a record number wider than the RELATIVE KEY ending the
/// reading (14). Every FILE STATUS the sort leaves is printed.
const SORT_KEYED: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T39CSRT.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT IDX-IN ASSIGN TO \"idxin.dat\"
        ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
        RECORD KEY IS II-KEY
        FILE STATUS IS WS-FS-IDX.
    SELECT REL-IN ASSIGN TO \"relin.dat\"
        ORGANIZATION IS RELATIVE ACCESS MODE IS RANDOM
        RELATIVE KEY IS WS-RIN
        FILE STATUS IS WS-FS-REL.
    SELECT SEQ-OUT ASSIGN TO \"seqout.txt\"
        ORGANIZATION IS LINE SEQUENTIAL
        FILE STATUS IS WS-FS-SEQ.
    SELECT REL-OUT ASSIGN TO \"relout.dat\"
        ORGANIZATION IS RELATIVE ACCESS MODE IS SEQUENTIAL
        RELATIVE KEY IS WS-ROUT
        FILE STATUS IS WS-FS-ROUT.
    SELECT REL-RANDOM ASSIGN TO \"relrnd.dat\"
        ORGANIZATION IS RELATIVE ACCESS MODE IS RANDOM
        RELATIVE KEY IS WS-RRND
        FILE STATUS IS WS-FS-ROUT.
    SELECT IDX-OUT ASSIGN TO \"idxout.dat\"
        ORGANIZATION IS INDEXED ACCESS MODE IS SEQUENTIAL
        RECORD KEY IS IO-KEY
        FILE STATUS IS WS-FS-IOUT.
    SELECT REL-WIDE ASSIGN TO \"relnine.dat\"
        ORGANIZATION IS RELATIVE ACCESS MODE IS RANDOM
        RELATIVE KEY IS WS-RWIDE.
    SELECT REL-NARROW ASSIGN TO \"relnine.dat\"
        ORGANIZATION IS RELATIVE ACCESS MODE IS SEQUENTIAL
        RELATIVE KEY IS WS-RNARROW
        FILE STATUS IS WS-FS-REL.
    SELECT SORT-FILE ASSIGN TO \"sortk.tmp\".
DATA DIVISION.
FILE SECTION.
FD IDX-IN.
01 II-REC.
   05 II-KEY  PIC 9(2).
   05 II-NAME PIC X(3).
FD REL-IN.
01 RI-REC PIC X(5).
FD SEQ-OUT.
01 SO-REC PIC X(5).
FD REL-OUT.
01 RO-REC PIC X(5).
FD REL-RANDOM.
01 RR-REC PIC X(5).
FD IDX-OUT.
01 IO-REC.
   05 IO-KEY  PIC 9(2).
   05 IO-NAME PIC X(3).
FD REL-WIDE.
01 RW-REC PIC X(5).
FD REL-NARROW.
01 RN-REC PIC X(5).
SD SORT-FILE.
01 SORT-REC.
   05 S-KEY  PIC 9(2).
   05 S-NAME PIC X(3).
WORKING-STORAGE SECTION.
01 WS-RIN     PIC 9(3).
01 WS-ROUT    PIC 9(3).
01 WS-RRND    PIC 9(3).
01 WS-RWIDE   PIC 9(2).
01 WS-RNARROW PIC 9.
01 WS-FS-IDX  PIC XX.
01 WS-FS-REL  PIC XX.
01 WS-FS-SEQ  PIC XX.
01 WS-FS-ROUT PIC XX.
01 WS-FS-IOUT PIC XX.
01 WS-EOF     PIC X.
PROCEDURE DIVISION.
    OPEN OUTPUT IDX-IN
    MOVE \"30CCC\" TO II-REC WRITE II-REC
    MOVE \"10AAA\" TO II-REC WRITE II-REC
    MOVE \"20BBB\" TO II-REC WRITE II-REC
    CLOSE IDX-IN
    OPEN OUTPUT REL-IN
    MOVE 5 TO WS-RIN MOVE \"50EEE\" TO RI-REC WRITE RI-REC
    MOVE 2 TO WS-RIN MOVE \"60FFF\" TO RI-REC WRITE RI-REC
    MOVE 7 TO WS-RIN MOVE \"40DDD\" TO RI-REC WRITE RI-REC
    CLOSE REL-IN
    SORT SORT-FILE ON DESCENDING KEY S-NAME
        USING IDX-IN GIVING SEQ-OUT
    DISPLAY \"indexed in \" WS-FS-IDX \" seq out \" WS-FS-SEQ
    PERFORM SHOW-SEQ
    MOVE 0 TO WS-ROUT
    SORT SORT-FILE ON ASCENDING KEY S-KEY
        USING REL-IN GIVING REL-OUT
    DISPLAY \"relative in \" WS-FS-REL \" out \" WS-FS-ROUT \" key \" WS-ROUT
    PERFORM SHOW-REL
    MOVE 0 TO WS-RRND
    SORT SORT-FILE ON DESCENDING KEY S-KEY
        USING IDX-IN GIVING REL-RANDOM
    DISPLAY \"random out \" WS-FS-ROUT \" key \" WS-RRND
    SORT SORT-FILE ON ASCENDING KEY S-KEY
        USING REL-IN IDX-IN GIVING IDX-OUT
    DISPLAY \"indexed out \" WS-FS-IOUT
    PERFORM SHOW-IDX
    MERGE SORT-FILE ON ASCENDING KEY S-KEY
        USING IDX-IN IDX-OUT GIVING REL-OUT
    DISPLAY \"merged out \" WS-FS-ROUT \" key \" WS-ROUT
    PERFORM SHOW-REL
    OPEN OUTPUT REL-WIDE
    MOVE 3 TO WS-RWIDE MOVE \"70GGG\" TO RW-REC WRITE RW-REC
    MOVE 12 TO WS-RWIDE MOVE \"80HHH\" TO RW-REC WRITE RW-REC
    CLOSE REL-WIDE
    SORT SORT-FILE ON ASCENDING KEY S-KEY
        USING REL-NARROW GIVING SEQ-OUT
    DISPLAY \"narrow key in \" WS-FS-REL
    PERFORM SHOW-SEQ
    STOP RUN.
SHOW-SEQ.
    MOVE \"N\" TO WS-EOF
    OPEN INPUT SEQ-OUT
    PERFORM UNTIL WS-EOF = \"Y\"
        READ SEQ-OUT
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"seq \" SO-REC
        END-READ
    END-PERFORM
    CLOSE SEQ-OUT.
SHOW-REL.
    MOVE \"N\" TO WS-EOF
    OPEN INPUT REL-OUT
    PERFORM UNTIL WS-EOF = \"Y\"
        READ REL-OUT NEXT
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"rel \" WS-ROUT \"=\" RO-REC
        END-READ
    END-PERFORM
    CLOSE REL-OUT.
SHOW-IDX.
    MOVE \"N\" TO WS-EOF
    OPEN INPUT IDX-OUT
    PERFORM UNTIL WS-EOF = \"Y\"
        READ IDX-OUT NEXT
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"idx \" IO-REC
        END-READ
    END-PERFORM
    CLOSE IDX-OUT.
";

/// SORT and MERGE (T39): USING and GIVING, two GIVING files (one LINE SEQUENTIAL), USING with an
/// OUTPUT PROCEDURE, an INPUT PROCEDURE with GIVING, MERGE with GIVING and with an OUTPUT PROCEDURE,
/// variable-length records kept at their own length, equal keys in release order, and a USING file
/// that is absent (its OPEN 35, then its CLOSE 42). Every FILE STATUS the sort leaves is printed.
const SORTING: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T39SRT.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT IN-FILE ASSIGN TO \"in.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS-IN.
    SELECT OUT-FILE ASSIGN TO \"out.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS-OUT.
    SELECT OUT-COPY ASSIGN TO \"copy.txt\"
        ORGANIZATION IS LINE SEQUENTIAL
        FILE STATUS IS WS-FS-CPY.
    SELECT MRG-A ASSIGN TO \"a.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS-IN.
    SELECT MRG-B ASSIGN TO \"b.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS-IN.
    SELECT VAR-IN ASSIGN TO \"varin.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS-IN.
    SELECT VAR-OUT ASSIGN TO \"varout.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS-OUT.
    SELECT MISSING-FILE ASSIGN TO \"missing.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS-IN.
    SELECT SORT-FILE ASSIGN TO \"sort.tmp\".
    SELECT VSORT-FILE ASSIGN TO \"vsort.tmp\".
DATA DIVISION.
FILE SECTION.
FD IN-FILE.
01 IN-REC.
   05 IN-CITY PIC X(8).
   05 IN-AMT  PIC 9(4).
   05 IN-NAME PIC X(8).
FD OUT-FILE.
01 OUT-REC PIC X(20).
FD OUT-COPY.
01 COPY-REC PIC X(20).
FD MRG-A.
01 A-REC PIC X(20).
FD MRG-B.
01 B-REC PIC X(20).
FD VAR-IN
   RECORD VARYING FROM 4 TO 30 CHARACTERS DEPENDING ON WS-VLEN.
01 VIN-REC PIC X(30).
FD VAR-OUT
   RECORD VARYING FROM 4 TO 30 CHARACTERS DEPENDING ON WS-VLEN.
01 VOUT-REC PIC X(30).
FD MISSING-FILE.
01 MISSING-REC PIC X(20).
SD SORT-FILE.
01 SORT-REC.
   05 S-CITY PIC X(8).
   05 S-AMT  PIC 9(4).
   05 S-NAME PIC X(8).
SD VSORT-FILE.
01 VSORT-REC.
   05 VS-KEY PIC X(3).
   05 VS-TXT PIC X(27).
WORKING-STORAGE SECTION.
01 WS-FS-IN  PIC XX.
01 WS-FS-OUT PIC XX.
01 WS-FS-CPY PIC XX.
01 WS-EOF    PIC X.
01 WS-VLEN   PIC 99.
01 WS-COUNT  PIC 99.
PROCEDURE DIVISION.
MAIN-PARA.
    OPEN OUTPUT IN-FILE
    MOVE \"RIO     0300ANA     \" TO IN-REC
    WRITE IN-REC
    MOVE \"SAO     0100BRUNO   \" TO IN-REC
    WRITE IN-REC
    MOVE \"RIO     0900CARLA   \" TO IN-REC
    WRITE IN-REC
    MOVE \"BELEM   0500DAVI    \" TO IN-REC
    WRITE IN-REC
    MOVE \"SAO     0700EVA     \" TO IN-REC
    WRITE IN-REC
    MOVE \"RIO     0300FABIO   \" TO IN-REC
    WRITE IN-REC
    CLOSE IN-FILE
    SORT SORT-FILE ON ASCENDING KEY S-CITY DESCENDING KEY S-AMT
        USING IN-FILE GIVING OUT-FILE OUT-COPY
    DISPLAY \"using giving in \" WS-FS-IN \" out \" WS-FS-OUT \" copy \" WS-FS-CPY
    PERFORM SHOW-OUT
    OPEN INPUT OUT-COPY
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ OUT-COPY
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"copy [\" COPY-REC \"]\"
        END-READ
    END-PERFORM
    CLOSE OUT-COPY
    SORT SORT-FILE ON DESCENDING KEY S-NAME
        USING IN-FILE OUTPUT PROCEDURE IS TAKE-SORTED
    DISPLAY \"using output procedure \" WS-FS-IN
    SORT SORT-FILE ON ASCENDING KEY S-AMT
        INPUT PROCEDURE IS GIVE-RECORDS GIVING OUT-FILE
    DISPLAY \"input procedure giving \" WS-FS-OUT
    PERFORM SHOW-OUT
    OPEN OUTPUT MRG-A
    MOVE \"BELEM   0001A1      \" TO A-REC
    WRITE A-REC
    MOVE \"RIO     0002A2      \" TO A-REC
    WRITE A-REC
    MOVE \"SAO     0003A3      \" TO A-REC
    WRITE A-REC
    CLOSE MRG-A
    OPEN OUTPUT MRG-B
    MOVE \"CURITIBA0004B1      \" TO B-REC
    WRITE B-REC
    MOVE \"RIO     0005B2      \" TO B-REC
    WRITE B-REC
    CLOSE MRG-B
    MERGE SORT-FILE ON ASCENDING KEY S-CITY
        USING MRG-A MRG-B GIVING OUT-FILE
    DISPLAY \"merge giving \" WS-FS-IN \" \" WS-FS-OUT
    PERFORM SHOW-OUT
    MERGE SORT-FILE ON DESCENDING KEY S-CITY
        USING MRG-A MRG-B OUTPUT PROCEDURE IS TAKE-SORTED
    OPEN OUTPUT VAR-IN
    MOVE 4 TO WS-VLEN
    MOVE \"BBB4\" TO VIN-REC
    WRITE VIN-REC
    MOVE 30 TO WS-VLEN
    MOVE \"CCC thirty bytes, the longest.\" TO VIN-REC
    WRITE VIN-REC
    MOVE 12 TO WS-VLEN
    MOVE \"AAA twelve..\" TO VIN-REC
    WRITE VIN-REC
    CLOSE VAR-IN
    SORT VSORT-FILE ON DESCENDING KEY VS-KEY
        USING VAR-IN GIVING VAR-OUT
    DISPLAY \"varying \" WS-FS-IN \" \" WS-FS-OUT
    OPEN INPUT VAR-OUT
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ VAR-OUT
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"var \" WS-VLEN \" [\" VOUT-REC \"]\"
        END-READ
    END-PERFORM
    CLOSE VAR-OUT
    SORT SORT-FILE ON ASCENDING KEY S-CITY
        USING MISSING-FILE GIVING OUT-FILE
    DISPLAY \"missing using \" WS-FS-IN \" out \" WS-FS-OUT
    PERFORM SHOW-OUT
    STOP RUN.
SHOW-OUT.
    MOVE 0 TO WS-COUNT
    OPEN INPUT OUT-FILE
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ OUT-FILE
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END ADD 1 TO WS-COUNT
                       DISPLAY \"out \" WS-COUNT \" [\" OUT-REC \"]\"
        END-READ
    END-PERFORM
    CLOSE OUT-FILE
    DISPLAY \"out records \" WS-COUNT.
TAKE-SORTED.
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        RETURN SORT-FILE
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"returned [\" SORT-REC \"]\"
        END-RETURN
    END-PERFORM.
GIVE-RECORDS.
    MOVE \"X       0042LAST    \" TO SORT-REC
    RELEASE SORT-REC
    MOVE \"Y       0007FIRST   \" TO SORT-REC
    RELEASE SORT-REC
    MOVE \"Z       0019MIDDLE  \" TO SORT-REC
    RELEASE SORT-REC.
";

/// UNLOCK, COMMIT and ROLLBACK (T39) on an INDEXED file opened I-O, each followed by what the file
/// then holds: they act only on an INDEXED file's engine and set no FILE STATUS.
/// `UNLOCK` is not a reserved word to the front end, so an open operand list (`DISPLAY`, `OPEN`) would
/// take it as operands: the sentence before each `UNLOCK` ends with a period.
const TRANSACTIONS: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T39TX.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT ITEMS ASSIGN TO \"items.idx\"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS DYNAMIC
        RECORD KEY IS IT-ID
        FILE STATUS IS WS-FS.
    SELECT LOG-FILE ASSIGN TO \"log.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD ITEMS.
01 IT-REC.
   05 IT-ID   PIC 9(3).
   05 IT-NAME PIC X(7).
FD LOG-FILE.
01 LOG-REC PIC X(10).
WORKING-STORAGE SECTION.
01 WS-FS  PIC XX.
01 WS-EOF PIC X.
PROCEDURE DIVISION.
MAIN-PARA.
    OPEN OUTPUT ITEMS
    MOVE 1 TO IT-ID
    MOVE \"ONE\" TO IT-NAME
    WRITE IT-REC
    CLOSE ITEMS
    OPEN I-O ITEMS
    MOVE 2 TO IT-ID
    MOVE \"TWO\" TO IT-NAME
    WRITE IT-REC
    DISPLAY \"write 2 \" WS-FS
    COMMIT
    DISPLAY \"commit \" WS-FS
    MOVE 3 TO IT-ID
    MOVE \"THREE\" TO IT-NAME
    WRITE IT-REC
    DISPLAY \"write 3 \" WS-FS
    ROLLBACK
    DISPLAY \"rollback \" WS-FS
    MOVE 1 TO IT-ID
    READ ITEMS
    DISPLAY \"read 1 \" WS-FS \" \" IT-NAME.
    UNLOCK ITEMS
    DISPLAY \"unlock \" WS-FS
    PERFORM SHOW-ITEMS
    CLOSE ITEMS
    OPEN OUTPUT LOG-FILE.
    UNLOCK LOG-FILE
    DISPLAY \"unlock sequential \" WS-FS
    CLOSE LOG-FILE
    OPEN INPUT ITEMS
    DISPLAY \"after close\"
    PERFORM SHOW-ITEMS
    CLOSE ITEMS
    STOP RUN.
SHOW-ITEMS.
    MOVE 0 TO IT-ID
    START ITEMS KEY IS NOT LESS THAN IT-ID
    MOVE \"N\" TO WS-EOF
    PERFORM UNTIL WS-EOF = \"Y\"
        READ ITEMS NEXT RECORD
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"item \" IT-ID \" \" IT-NAME
        END-READ
    END-PERFORM.
";

/// LINAGE (T39b): the page a WRITE moves down, which the module keeps as the interpreter does. A
/// page with a stated body and footing, and one whose body and footing are data items (a footing
/// item of 0 falls back to the stated number, so every line is past it, until the program sets
/// it); ADVANCING by a number, by a data item and PAGE, and no phrase; AT END-OF-PAGE and NOT AT
/// END-OF-PAGE; LINAGE-COUNTER displayed, moved and compared; a reopened file starts a new page,
/// and a failed OPEN leaves the counter alone. The file's bytes do not change with the page.
const LINAGE: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T39BLIN.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT REPORT-A ASSIGN TO \"page-a.txt\"
        ORGANIZATION IS LINE SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT REPORT-B ASSIGN TO \"page-b.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT MISSING-REPORT ASSIGN TO \"absent.txt\"
        ORGANIZATION IS LINE SEQUENTIAL
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD REPORT-A
    LINAGE IS 5 LINES WITH FOOTING AT 4 LINES AT TOP 1 LINES AT BOTTOM 1.
01 A-LINE PIC X(20).
FD REPORT-B
    LINAGE IS WS-BODY LINES WITH FOOTING AT WS-FOOT.
01 B-LINE PIC X(12).
FD MISSING-REPORT
    LINAGE IS 10 LINES.
01 M-LINE PIC X(8).
WORKING-STORAGE SECTION.
01 WS-FS   PIC XX.
01 WS-BODY PIC 99 VALUE 3.
01 WS-FOOT PIC 99 VALUE 0.
01 WS-STEP PIC 9 VALUE 2.
01 WS-LC   PIC 999.
01 WS-ED   PIC -9(18).
01 WS-EZ   PIC ZZZ9.
01 WS-I    PIC 99.
PROCEDURE DIVISION.
MAIN-PARA.
    OPEN OUTPUT REPORT-A
    DISPLAY \"open a \" WS-FS \" lc \" LINAGE-COUNTER
    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 7
        MOVE WS-I TO A-LINE
        WRITE A-LINE AFTER ADVANCING 1 LINE
            AT END-OF-PAGE DISPLAY \"a \" WS-I \" eop lc \" LINAGE-COUNTER
            NOT AT END-OF-PAGE DISPLAY \"a \" WS-I \" lc \" LINAGE-COUNTER
        END-WRITE
    END-PERFORM
    WRITE A-LINE AFTER ADVANCING 2 LINES
        AT EOP DISPLAY \"a two eop lc \" LINAGE-COUNTER
    END-WRITE
    MOVE LINAGE-COUNTER TO WS-LC
    DISPLAY \"moved \" WS-LC
    MOVE LINAGE-COUNTER TO WS-ED WS-EZ
    DISPLAY \"edited \" WS-ED \" \" WS-EZ
    WRITE A-LINE BEFORE ADVANCING PAGE
    DISPLAY \"page lc \" LINAGE-COUNTER
    WRITE A-LINE AFTER ADVANCING WS-STEP LINES
    IF LINAGE-COUNTER = 3
        DISPLAY \"item step lc 3\"
    ELSE
        DISPLAY \"item step lc \" LINAGE-COUNTER
    END-IF
    WRITE A-LINE
    DISPLAY \"no phrase lc \" LINAGE-COUNTER
    CLOSE REPORT-A
    OPEN OUTPUT REPORT-A
    DISPLAY \"reopen \" WS-FS \" lc \" LINAGE-COUNTER
    CLOSE REPORT-A
    OPEN OUTPUT REPORT-B
    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 5
        MOVE \"B\" TO B-LINE
        WRITE B-LINE
            AT EOP DISPLAY \"b \" WS-I \" eop lc \" LINAGE-COUNTER
            NOT AT EOP DISPLAY \"b \" WS-I \" lc \" LINAGE-COUNTER
        END-WRITE
    END-PERFORM
    MOVE 2 TO WS-FOOT
    MOVE 4 TO WS-BODY
    PERFORM VARYING WS-I FROM 6 BY 1 UNTIL WS-I > 8
        WRITE B-LINE
            AT EOP DISPLAY \"b \" WS-I \" eop lc \" LINAGE-COUNTER
            NOT AT EOP DISPLAY \"b \" WS-I \" lc \" LINAGE-COUNTER
        END-WRITE
    END-PERFORM
    CLOSE REPORT-B
    OPEN INPUT MISSING-REPORT
    DISPLAY \"missing \" WS-FS \" lc \" LINAGE-COUNTER
    STOP RUN.
";

/// A PERFORM inside AT END-OF-PAGE and NOT AT END-OF-PAGE: the paragraph performed writes the footing
/// lines to the same file and returns, as any performed paragraph does (NIST SQ201M ran on past it into
/// the paragraphs after it, until 2.0.552). The paragraph after it is never reached.
const EOP_PERFORM: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T40EOPP.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT REPORT-F ASSIGN TO \"foot.txt\"
        ORGANIZATION IS LINE SEQUENTIAL
        FILE STATUS IS WS-FS.
DATA DIVISION.
FILE SECTION.
FD REPORT-F
    LINAGE IS 6 LINES WITH FOOTING AT 4.
01 F-LINE PIC X(16).
WORKING-STORAGE SECTION.
01 WS-FS   PIC XX.
01 WS-I    PIC 99.
01 WS-FEET PIC 99 VALUE 0.
PROCEDURE DIVISION.
MAIN-PARA.
    OPEN OUTPUT REPORT-F
    PERFORM VARYING WS-I FROM 1 BY 1 UNTIL WS-I > 9
        MOVE WS-I TO F-LINE
        WRITE F-LINE
            AT END-OF-PAGE PERFORM FOOTING-LINE 2 TIMES
            NOT AT END-OF-PAGE PERFORM BODY-LINE
        END-WRITE
    END-PERFORM
    CLOSE REPORT-F
    DISPLAY \"footings \" WS-FEET \" status \" WS-FS
    STOP RUN.
BODY-LINE.
    DISPLAY \"body \" WS-I \" lc \" LINAGE-COUNTER.
FOOTING-LINE.
    ADD 1 TO WS-FEET
    MOVE \"footing\" TO F-LINE
    WRITE F-LINE
    DISPLAY \"footing \" WS-FEET \" lc \" LINAGE-COUNTER.
NOT-PERFORMED.
    DISPLAY \"fell through\".
";

/// A MERGE whose OUTPUT PROCEDURE no SORT shares: the procedure, a section whose paragraphs leave by
/// GO TO, returns at its end to the statement after the MERGE (NIST ST140A and ST144A ran on past it,
/// through the sections after it, until 2.0.553). The section after it is never reached.
const MERGE_PROCEDURE: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T40MRGP.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT MRG-A ASSIGN TO \"mrga.dat\"
        ORGANIZATION IS SEQUENTIAL.
    SELECT MRG-B ASSIGN TO \"mrgb.dat\"
        ORGANIZATION IS SEQUENTIAL.
    SELECT MRG-OUT ASSIGN TO \"merged.txt\"
        ORGANIZATION IS LINE SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT WORK-FILE ASSIGN TO \"mrgwork.tmp\".
DATA DIVISION.
FILE SECTION.
FD MRG-A.
01 A-REC PIC X(8).
FD MRG-B.
01 B-REC PIC X(8).
FD MRG-OUT.
01 O-REC PIC X(8).
SD WORK-FILE.
01 W-REC.
    05 W-KEY PIC X(4).
    05 W-TAG PIC X(4).
WORKING-STORAGE SECTION.
01 WS-FS PIC XX.
01 WS-N  PIC 99 VALUE 0.
PROCEDURE DIVISION.
MAIN-LINE SECTION.
MAIN-PARA.
    OPEN OUTPUT MRG-A
    MOVE \"A001AAAA\" TO A-REC
    WRITE A-REC
    MOVE \"C003AAAA\" TO A-REC
    WRITE A-REC
    CLOSE MRG-A
    OPEN OUTPUT MRG-B
    MOVE \"B002BBBB\" TO B-REC
    WRITE B-REC
    MOVE \"D004BBBB\" TO B-REC
    WRITE B-REC
    CLOSE MRG-B
    MERGE WORK-FILE ON ASCENDING KEY W-KEY
        USING MRG-A MRG-B
        OUTPUT PROCEDURE IS TAKE-MERGED
    DISPLAY \"after merge \" WS-N \" status \" WS-FS
    STOP RUN.
TAKE-MERGED SECTION.
OPEN-OUT.
    OPEN OUTPUT MRG-OUT.
RETURN-NEXT.
    RETURN WORK-FILE
        AT END GO TO CLOSE-OUT.
    ADD 1 TO WS-N
    DISPLAY \"merged \" WS-N \" [\" W-REC \"]\"
    WRITE O-REC FROM W-REC
    GO TO RETURN-NEXT.
CLOSE-OUT.
    CLOSE MRG-OUT.
NOT-PERFORMED SECTION.
FELL-THROUGH.
    DISPLAY \"fell through\".
";


const SAME_AREA: &str = "\
IDENTIFICATION DIVISION.
PROGRAM-ID. T39BSAME.
ENVIRONMENT DIVISION.
INPUT-OUTPUT SECTION.
FILE-CONTROL.
    SELECT IN-A ASSIGN TO \"same-a.dat\"
        ORGANIZATION IS SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT IN-B ASSIGN TO \"same-b.txt\"
        ORGANIZATION IS LINE SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT IN-C ASSIGN TO \"same-c.dat\"
        ORGANIZATION IS RELATIVE
        ACCESS MODE IS SEQUENTIAL
        FILE STATUS IS WS-FS.
    SELECT IN-D ASSIGN TO \"same-d.idx\"
        ORGANIZATION IS INDEXED
        ACCESS MODE IS DYNAMIC
        RECORD KEY IS D-KEY
        FILE STATUS IS WS-FS.
    SELECT SORT-W ASSIGN TO \"same.tmp\".
I-O-CONTROL.
    SAME RECORD AREA FOR IN-A IN-B IN-C
    SAME RECORD AREA FOR SORT-W IN-D.
DATA DIVISION.
FILE SECTION.
FD IN-A.
01 A-REC.
   05 A-KEY  PIC X(4).
   05 A-TEXT PIC X(8).
FD IN-B.
01 B-REC PIC X(20).
FD IN-C.
01 C-REC PIC X(6).
FD IN-D.
01 D-REC.
   05 D-KEY  PIC X(4).
   05 D-TEXT PIC X(10).
SD SORT-W.
01 W-REC.
   05 W-KEY  PIC X(4).
   05 W-REST PIC X(12).
WORKING-STORAGE SECTION.
01 WS-FS   PIC XX.
01 WS-EOF  PIC X VALUE \"N\".
01 WS-HOLD PIC X(20).
PROCEDURE DIVISION.
MAIN-PARA.
    OPEN OUTPUT IN-A
    MOVE \"A001\" TO A-KEY
    MOVE \"ALPHA\" TO A-TEXT
    WRITE A-REC
    MOVE \"A002\" TO A-KEY
    MOVE \"BETA\" TO A-TEXT
    WRITE A-REC
    CLOSE IN-A
    OPEN OUTPUT IN-B
    MOVE \"LINE-B-ONE-IS-TWENTY\" TO B-REC
    WRITE B-REC
    MOVE \"SHORT B\" TO B-REC
    WRITE B-REC
    CLOSE IN-B
    OPEN OUTPUT IN-C
    MOVE \"CCC111\" TO C-REC
    WRITE C-REC
    CLOSE IN-C
    OPEN OUTPUT IN-D
    MOVE \"K003\" TO D-KEY
    MOVE \"THIRD\" TO D-TEXT
    WRITE D-REC
    MOVE \"K001\" TO D-KEY
    MOVE \"FIRST\" TO D-TEXT
    WRITE D-REC
    MOVE \"K002\" TO D-KEY
    MOVE \"SECOND\" TO D-TEXT
    WRITE D-REC
    CLOSE IN-D
    OPEN INPUT IN-A IN-B IN-C
    READ IN-A
    DISPLAY \"read a \" WS-FS \" b [\" B-REC \"] c [\" C-REC \"]\"
    READ IN-B
    DISPLAY \"read b \" WS-FS \" a [\" A-REC \"] c [\" C-REC \"]\"
    READ IN-B
    DISPLAY \"short b \" WS-FS \" a [\" A-REC \"] c [\" C-REC \"]\"
    READ IN-C INTO WS-HOLD
    DISPLAY \"read c \" WS-FS \" a [\" A-REC \"] b [\" B-REC \"] hold [\" WS-HOLD \"]\"
    READ IN-C
        AT END DISPLAY \"c at end \" WS-FS \" a [\" A-REC \"] b [\" B-REC \"]\"
    END-READ
    READ IN-A
    DISPLAY \"second a \" WS-FS \" b [\" B-REC \"] key \" A-KEY
    CLOSE IN-A IN-B IN-C
    SORT SORT-W ON DESCENDING KEY W-KEY
        INPUT PROCEDURE IS FEED
        OUTPUT PROCEDURE IS DRAIN
    STOP RUN.
FEED.
    OPEN INPUT IN-D
    PERFORM UNTIL WS-FS NOT = \"00\"
        READ IN-D NEXT
            AT END CONTINUE
            NOT AT END
                DISPLAY \"feed [\" W-REC \"]\"
                RELEASE W-REC
        END-READ
    END-PERFORM
    CLOSE IN-D.
DRAIN.
    PERFORM UNTIL WS-EOF = \"Y\"
        RETURN SORT-W
            AT END MOVE \"Y\" TO WS-EOF
            NOT AT END DISPLAY \"drain [\" W-REC \"]\"
        END-RETURN
    END-PERFORM.
";

/// The programs, by organisation, with lines each must print: the statuses the cases are there for.
const PROGRAMS: &[(&str, &str, &[&str])] = &[
    (
        "sequential",
        SEQUENTIAL,
        &[
            "open of an open file 41",
            "close of a closed file 42",
            "rewrite 00",
            "rewrite with no read 43",
            "past the end 46",
            "open after lock 38",
            "absent 35",
            "absent optional 05",
            "var write 3 44",
            "var 25 [twenty-five bytes long...",
        ],
    ),
    ("relative", RELATIVE, &["write 10 key 0 00", "end 14", "write 5 again invalid 22", "read 3 invalid 23", "read 9 invalid 23", "next 0005 105 FIVE"]),
    (
        "indexed",
        INDEXED,
        &["write 10 again invalid 22", "read 99 invalid 23", "by id 00020 BRUNO SILVA", "by city RIO        00040 DAVI", "seq write 4 out of order 21", "seq 005 FIVE"],
    ),
    (
        "sort",
        SORTING,
        &[
            "using giving in 00 out 00 copy 00",
            "out 01 [BELEM   0500DAVI    ]",
            "out 03 [RIO     0300ANA     ]",
            "out 04 [RIO     0300FABIO   ]",
            "copy [SAO     0100BRUNO   ]",
            "returned [SAO     0100BRUNO   ]",
            "out 01 [Y       0007FIRST   ]",
            "out 02 [CURITIBA0004B1      ]",
            "var 30 [CCC thirty bytes, the longest.]",
            "var 04 [BBB4",
            "missing using 42 out 00",
            "out records 00",
        ],
    ),
    (
        "sort keyed",
        SORT_KEYED,
        &[
            "indexed in 00 seq out 00",
            "seq 30CCC",
            "relative in 00 out 00 key 003",
            "rel 001=40DDD",
            "rel 003=60FFF",
            "random out 00 key 003",
            "indexed out 00",
            "idx 60FFF",
            "merged out 00 key 009",
            "rel 002=10AAA",
            "rel 009=60FFF",
            "narrow key in 00",
            "seq 70GGG",
        ],
    ),
    ("transactions", TRANSACTIONS, &["commit 00", "rollback 00", "unlock sequential 00", "after close"]),
    (
        "linage",
        LINAGE,
        &[
            "open a 00 lc 1",
            "a 03 lc 3",
            "a 04 eop lc 4",
            "a 06 lc 1",
            "a two eop lc 4",
            "moved 004",
            "edited  000000000000000004    4",
            "page lc 1",
            "item step lc 3",
            "no phrase lc 4",
            "reopen 00 lc 1",
            "b 01 eop lc 1",
            "b 04 eop lc 1",
            "b 07 eop lc 4",
            "b 08 lc 1",
            "missing 35 lc 1",
        ],
    ),
    (
        "eop perform",
        EOP_PERFORM,
        &["body 01 lc 1", "footing 01 lc 5", "footing 02 lc 6", "body 05 lc 1", "footing 04 lc 6", "body 09 lc 1", "footings 04 status 00"],
    ),
    (
        "merge procedure",
        MERGE_PROCEDURE,
        &["merged 01 [A001AAAA]", "merged 02 [B002BBBB]", "merged 03 [C003AAAA]", "merged 04 [D004BBBB]", "after merge 04 status 00"],
    ),
    (
        "same area",
        SAME_AREA,
        &[
            "read a 00 b [A001ALPHA           ] c [A001AL]",
            "read b 00 a [LINE-B-ONE-I] c [LINE-B]",
            "short b 00 a [SHORT B     ] c [SHORT ]",
            "read c 00 a [CCC111      ] b [CCC111              ] hold [CCC111              ]",
            "c at end 10 a [CCC111      ] b [CCC111              ]",
            "second a 00 b [A002BETA            ] key A002",
            "feed [K001FIRST       ]",
            "feed [K003THIRD       ]",
            "drain [K003THIRD       ]",
            "drain [K001FIRST       ]",
        ],
    ),
];

/// A folder of its own under the system's temporary folder, removed at the end.
struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        let p = std::env::temp_dir().join(format!("prc-runner-t38-{tag}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&p);
        std::fs::create_dir_all(&p).unwrap();
        Scratch(p)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        if std::env::var_os("PRC_RUNNER_KEEP").is_none() {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }
}

fn parse(src: &str) -> cobolt_ast::program::Program {
    let parsed = cobolt_parser::parse(cobolt_lexer::tokenize(src, cobolt_lexer::SourceFormat::Free));
    let errors: Vec<String> = parsed.diagnostics.iter().filter(|d| d.is_error()).map(|d| format!("line {}: {}", d.span.line, d.message)).collect();
    assert!(errors.is_empty(), "the program does not parse: {errors:?}");
    parsed.program.expect("a program")
}

/// `rcrun`, built now from the current source (a stale binary would compare against old code).
fn rcrun() -> PathBuf {
    let cargo = std::env::var("CARGO").unwrap_or_else(|_| "cargo".into());
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let status = Command::new(cargo).current_dir(&root).args(["build", "-q", "-p", "cobolt-cli", "--bin", "rcrun"]).status().expect("run cargo");
    assert!(status.success(), "cargo build -p cobolt-cli --bin rcrun failed");
    let exe = std::env::current_exe().unwrap();
    let dir = exe.parent().and_then(Path::parent).unwrap();
    dir.join(format!("rcrun{}", std::env::consts::EXE_SUFFIX))
}

fn run(mut cmd: Command, cache: &Path) -> (Output, f64) {
    cmd.env("PRC_RUNNER_CACHE", cache).env("LC_ALL", "en_US.UTF-8").env_remove("COBOL_INDEXED_ENGINE");
    cmd.stdin(Stdio::piped()).stdout(Stdio::piped()).stderr(Stdio::piped());
    let t = Instant::now();
    let mut child = cmd.spawn().expect("start the program");
    let _ = child.stdin.take().unwrap().write_all(b"");
    let out = child.wait_with_output().unwrap();
    (out, t.elapsed().as_secs_f64() * 1000.0)
}

/// Every file a run left in `dir`, by name, but the program's own source.
fn files_in(dir: &Path) -> BTreeMap<String, Vec<u8>> {
    std::fs::read_dir(dir)
        .unwrap()
        .filter_map(Result::ok)
        .filter(|e| e.path().is_file())
        .map(|e| (e.file_name().to_string_lossy().into_owned(), std::fs::read(e.path()).unwrap()))
        .filter(|(n, _)| !n.ends_with(".cbl") && !n.starts_with("prog"))
        .collect()
}

#[test]
fn files_of_every_organisation_match_the_interpreter() {
    let root = Scratch::new("files");
    let cache = root.0.join("cache");
    let t = Instant::now();
    let rcrun = rcrun();
    let rcrun_build_ms = t.elapsed().as_secs_f64() * 1000.0;
    let runner = std::fs::read(env!("CARGO_BIN_EXE_prc-runner")).expect("the built prc-runner");

    let mut checks = 0;
    let mut rows = Vec::new();
    for (tag, src, expected) in PROGRAMS {
        let t = Instant::now();
        let opts = cobolt_wasm::Options { native: true, ..cobolt_wasm::Options::default() };
        let module = cobolt_wasm::compile_program(&parse(src), &opts).unwrap_or_else(|e| panic!("{tag}: the generator refused the program: {e}")).wasm;
        let compile_ms = t.elapsed().as_secs_f64() * 1000.0;
        let scan = cobolt_wasm_abi::scan::scan_module(&module).unwrap();
        let caps = scan.embedded_manifest().unwrap().capabilities;
        assert!(caps.iter().any(|c| c.starts_with("files.")), "{tag}: the manifest asks for a file capability: {caps:?}");
        assert!(scan.custom(cobolt_wasm_abi::files::FILES_SECTION).is_some(), "{tag}: the module carries its files");
        checks += 2;

        let ours = root.0.join(format!("{tag}-runner"));
        let theirs = root.0.join(format!("{tag}-rcrun"));
        std::fs::create_dir_all(&ours).unwrap();
        std::fs::create_dir_all(&theirs).unwrap();
        let exe = ours.join(format!("prog{}", std::env::consts::EXE_SUFFIX));
        let payload = cobolt_program_package::encode(&[(Kind::Module, &module)]);
        let packaged = cobolt_program_package::embed(&runner, &payload, "prc-t38-files").expect("the runner takes a payload");
        cobolt_program_package::write_executable(&exe, &packaged).expect("write the program");
        let source = theirs.join(format!("{tag}.cbl"));
        std::fs::write(&source, src).unwrap();

        let mut a = Command::new(&exe);
        a.current_dir(&ours);
        let (runner_out, runner_ms) = run(a, &cache);
        let mut b = Command::new(&rcrun);
        b.arg("run").arg(&source).current_dir(&theirs);
        let (rcrun_out, rcrun_ms) = run(b, &cache);

        let said = |o: &Output| String::from_utf8_lossy(&o.stdout).into_owned();
        assert!(runner_out.status.success(), "{tag}: the runner failed: {}", String::from_utf8_lossy(&runner_out.stderr));
        assert!(rcrun_out.status.success(), "{tag}: rcrun run failed: {}", String::from_utf8_lossy(&rcrun_out.stderr));
        assert_eq!(said(&runner_out), said(&rcrun_out), "{tag}: the runner prints what rcrun run prints\nrunner stderr: {}", String::from_utf8_lossy(&runner_out.stderr));
        checks += 3;
        let lines = said(&runner_out).lines().count();
        for e in *expected {
            assert!(said(&runner_out).contains(e), "{tag}: the run prints `{e}`:\n{}", said(&runner_out));
            checks += 1;
        }

        let left = files_in(&ours);
        let right = files_in(&theirs);
        assert_eq!(left.keys().collect::<Vec<_>>(), right.keys().collect::<Vec<_>>(), "{tag}: the same files are made");
        checks += 1;
        let mut bytes = 0;
        for (name, data) in &left {
            assert!(data == &right[name], "{tag}: {name} differs ({} bytes from the runner, {} from rcrun run)", data.len(), right[name].len());
            bytes += data.len();
            checks += 1;
        }
        rows.push(format!(
            "  {tag:<10}: {lines:>2} lines printed, {} files ({bytes} bytes) equal; generate {compile_ms:.1} ms, runner {runner_ms:.1} ms, rcrun run {rcrun_ms:.1} ms",
            left.len()
        ));
    }

    println!("── files_of_every_organisation_match_the_interpreter ──");
    println!("  sequential: OUTPUT/EXTEND/I-O/INPUT, REWRITE (00, 43), past the end (46), open file (41), closed file (42),");
    println!("              WITH LOCK (38), absent (35), OPTIONAL absent (05), path in a data item, LINE SEQUENTIAL,");
    println!("              VARYING 5 TO 40 DEPENDING ON (10, 25 written; 3 out of range 44)");
    println!("  relative:   sequential creator (keys 1-9, then 10 and 11 on a 1-digit key), READ to 14,");
    println!("              dynamic WRITE/duplicate (22)/READ/missing (23)/REWRITE/DELETE/START/READ NEXT");
    println!("  indexed:    dynamic with alternate key WITH DUPLICATES, duplicate (22), missing (23), REWRITE, DELETE,");
    println!("              START on both keys, READ NEXT; sequential access out of order (21)");
    println!("  sort:       SORT USING/GIVING (two GIVING files, one LINE SEQUENTIAL), USING with an OUTPUT PROCEDURE,");
    println!("              INPUT PROCEDURE with GIVING, MERGE with GIVING and with an OUTPUT PROCEDURE, VARYING 4 TO 30");
    println!("              records at their own length, equal keys in release order, an absent USING file (35, then 42 at its CLOSE)");
    println!("  sort keyed: SORT/MERGE with an indexed USING file (key order), a relative one (past empty slots), relative GIVING");
    println!("              in the sequential and random access modes (slots 1, 2, 3, RELATIVE KEY on the last), indexed GIVING");
    println!("              in the sequential access mode, a MERGE of two indexed files, a record number wider than the RELATIVE KEY (14)");
    println!("  transactions: COMMIT, ROLLBACK, UNLOCK on an INDEXED file opened I-O; UNLOCK on a SEQUENTIAL file");
    println!("  linage:     LINAGE stated and in data items (a footing item of 0 falls back), ADVANCING 1, 2, PAGE, a data item");
    println!("              and none, AT/NOT AT END-OF-PAGE, LINAGE-COUNTER displayed/moved (also to numeric-edited items)/compared, reopen, a failed OPEN");
    println!("  eop perform: a PERFORM in AT END-OF-PAGE (2 TIMES, writing the footing to the same file) and in NOT AT END-OF-PAGE returns");
    println!("  merge procedure: a MERGE's OUTPUT PROCEDURE (a section left by GO TO, shared with no SORT) returns after it");
    println!("  same area:  SAME RECORD AREA across SEQUENTIAL, LINE SEQUENTIAL and RELATIVE files (longer peer padded, shorter cut),");
    println!("              READ INTO, AT END leaves the peers alone; an INDEXED file READ NEXT through its SORT peer, RELEASEd and RETURNed");
    for r in &rows {
        println!("{r}");
    }
    println!("  rcrun built in {rcrun_build_ms:.0} ms");
    println!("  checks: {checks} passed, 0 failed");
}
