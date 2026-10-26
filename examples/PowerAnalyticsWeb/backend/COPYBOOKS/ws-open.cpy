      *> Working storage of the paragraphs in p-db and p-filter, which every program of the Backend COPYs into
      *> its PROCEDURE DIVISION: the statements of one call must all run in ONE program, because the status
      *> items (SQLSTATE ...) of a program that a nested program CALLs are not the ones the statements update.
      *>
      *> OPN-OUT is what the shared paragraphs tell their program; it never travels:
      *>   OPN-STATUS  OK | NOFILE (no database and no data/fundos.sql) | SQLERR (OPN-MESSAGE: SQLSTATE and the database's words)
      *>   OPN-LOAD    READY (the database was there) | LOADED (this call built it) | RACED (another call built it first)
      *>   OPN-LINES   statements the loader ran (0 when READY)
      *>   OPN-NAME-KEY / OPN-CAT-KEY   the typed texts folded like the nome_key column (NORM-TERM), trimmed
       01 OPN-OUT.
          05 OPN-STATUS                          PIC    X(8).
          05 OPN-MESSAGE                         PIC    X(160).
          05 OPN-LOAD                            PIC    X(8).
          05 OPN-LINES                           PIC    9(6).
          05 OPN-NAME-KEY                        PIC    X(240).
          05 OPN-CAT-KEY                         PIC    X(240).
       01 SH-FS                                  PIC    XX.
       01 SH-HAS                                 PIC    9(9).
       01 SH-FAILED                              PIC    9.
       01 SH-STATE                               PIC    X(5).
       01 SH-WHY                                 PIC    X(160).
       01 SH-I                                   PIC    9(4).
       01 SH-ONE                                 PIC    X(210).
       01 SH-TERM-N                              PIC    X(80).
       01 SH-TERM-C                              PIC    X(80).
       01 SH-NAME-MODE                           PIC    9.
       01 SH-CAT-MODE                            PIC    9.
       01 SH-DATE-ON                             PIC    9.
       01 SH-DATE-LO                             PIC    X(10).
       01 SH-DATE-HI                             PIC    X(10).
       01 SH-PL-ON                               PIC    9.
       01 SH-PL-LO                               PIC    9(15).
       01 SH-PL-HI                               PIC    9(15).
       01 SH-N2-ON                               PIC    9.
       01 SH-N3-ON                               PIC    9.
       01 SH-SLOT-N                              PIC    99.
       01 SH-SLOT-NAME                           PIC    X(60).
