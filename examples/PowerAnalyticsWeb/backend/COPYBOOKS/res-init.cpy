      *> INI-RES: the start-up facts of the database, what INIT-DB and LOAD-DIMS of the desktop leave behind.
      *> INI-STATUS    OK | NOFILE | SQLERR | TRUNC (a level list was longer than its table: INI-N2-TOTAL / INI-N3-TOTAL say by how much)
      *> INI-LOAD      READY (the database was there) | LOADED (this call built it from data/fundos.sql) | RACED
      *> INI-LINES     statements the loader ran (0 when READY)
      *> INI-UNIV      the universe: COUNT(*) of fundos (WS-UNIV)
      *> INI-REFDATE   newest data_ref as dd/mm/yyyy, blank when none (the date of the header caption)
      *> INI-CAT-N     categories in the table (WS-CATCNT, at most 10); INI-CAT-NAME(1..10) = WS-CAT-NAME, most funds first
      *> INI-N2-*      the level-2 names, most funds first (the items of LST-N2); INI-N2-TOTAL = how many exist
      *> INI-N3-*      the same for level 3 (LST-N3)
       01 INI-RES.
          05 INI-STATUS                          PIC    X(8).
          05 INI-MESSAGE                         PIC    X(160).
          05 INI-LOAD                            PIC    X(8).
          05 INI-LINES                           PIC    9(6).
          05 INI-UNIV                            PIC    9(9).
          05 INI-REFDATE                         PIC    X(10).
          05 INI-CAT-N                           PIC    99.
          05 INI-CAT-NAME                        PIC    X(24)    OCCURS 10.
          05 INI-N2-TOTAL                        PIC    9(4).
          05 INI-N2-N                            PIC    99.
          05 INI-N2-NAME                         PIC    X(60)
                                                 OCCURS 0 TO 64 DEPENDING ON INI-N2-N.
          05 INI-N3-TOTAL                        PIC    9(4).
          05 INI-N3-N                            PIC    99.
          05 INI-N3-NAME                         PIC    X(60)
                                                 OCCURS 0 TO 64 DEPENDING ON INI-N3-N.
