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
       01 SH-PICK-N                              PIC    9(9).
       01 SH-TERM-N                              PIC    X(80).
       01 SH-TERM-C                              PIC    X(80).
       01 SH-N2-ON                               PIC    9.
       01 SH-N3-ON                               PIC    9.
       01 SH-SLOT-N                              PIC    99.
       01 SH-SLOT-NAME                           PIC    X(60).
      *> The filter as the desktop's BUILD-WHERE writes it (OPN-MAKE-WHERE): the same text, "WHERE 1=1" and the
      *> same AND clauses in the same order, with a ? where the desktop pastes a value. The values are bound
      *> through the descriptor SQLDA below: OPN-WN of them belong to OPN-WHERE, a statement adds its own after.
      *> Every query program builds its statement as the desktop's Q-* procedure does, "... FROM fundos " +
      *> OPN-WHERE + its own tail, into OPN-SQL, so SQLite plans it exactly as it plans the desktop's: the same
      *> index, the same order of rows, the same order among equal sort keys, the same floating-point sums.
       01 OPN-WHERE                              PIC    X(600).
       01 OPN-WP                                 PIC    9(4).
       01 OPN-WN                                 PIC    9(4).
       01 OPN-SQL                                PIC    X(4000).
      *> One value to bind (OPN-ADD-PARAM appends it to SQLDA); the PL bounds as the desktop writes them (Z(14)9).
       01 OPN-PV                                 PIC    X(250).
       01 OPN-PLX                                PIC    Z(14)9.
      *> The descriptor of the values bound to the ? of a statement (OPEN ... USING DESCRIPTOR SQLDA): the layout of
      *> EXEC SQL INCLUDE SQLDA with room for 110 values instead of 100. The most a WHERE binds is 101 (a name text,
      *> two dates, two PL bounds, 32 level-2 and 64 level-3 names); FUND-YEARS adds 4 and FUND-GRID 2.
       01 SQLDA.
          05 SQLDA-TAG                           PIC    X(8)     VALUE "RCSQLDA1".
          05 SQLDA-CAPACITY                      PIC    S9(4)    COMP-5 VALUE 110.
          05 SQLDA-NEEDED                        PIC    S9(4)    COMP-5 VALUE 0.
          05 SQLDA-COUNT                         PIC    S9(4)    COMP-5 VALUE 0.
          05 SQLDA-ENTRY                         OCCURS 110 TIMES.
             10 SQLDA-NAME                       PIC    X(128).
             10 SQLDA-TYPE                       PIC    S9(4)    COMP-5.
             10 SQLDA-TYPE-NAME                  PIC    X(32).
             10 SQLDA-LENGTH                     PIC    S9(9)    COMP-5.
             10 SQLDA-PRECISION                  PIC    S9(4)    COMP-5.
             10 SQLDA-SCALE                      PIC    S9(4)    COMP-5.
             10 SQLDA-NULLABLE                   PIC    S9(4)    COMP-5.
             10 SQLDA-DATA                       USAGE  POINTER.
             10 SQLDA-IND-PTR                    USAGE  POINTER.
             10 SQLDA-IND                        PIC    S9(4)    COMP-5.
             10 SQLDA-VALUE                      PIC    X(250).
