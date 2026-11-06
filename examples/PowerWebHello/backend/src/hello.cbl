       IDENTIFICATION DIVISION.
       PROGRAM-ID. HELLO.
      *> PowerWebHello - the Backend's greeting. It is listed in backend/exports.toml,
      *> so the page can call it; it takes a name and answers with the greeting.

       DATA DIVISION.

       LINKAGE SECTION.
       01 LK-NAME                                PIC    X(30).
       01 LK-GREETING                            PIC    X(40).

       PROCEDURE DIVISION USING LK-NAME RETURNING LK-GREETING.
           MOVE    SPACES                        TO    LK-GREETING
           STRING  "Hello, " DELIMITED BY SIZE LK-NAME DELIMITED BY SPACE INTO LK-GREETING
           GOBACK.
