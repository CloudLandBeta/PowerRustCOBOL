      *> The filter of the dashboard, as TEMPORARY objects of the connection (they vanish with it at the end of the
      *> call; nothing is written to fundos.db):
      *>   pick (fld, name, n)   (a primary key, as the desktop's table) the ticked names: 'N' company names, 'C' categories, '2' level 2, '3' level 3,
      *>                         and 'T' the categories that the typed category text matches (BUILD-WHERE /
      *>                         CAT-CLAUSE read the table vis for that; here it is computed from the text)
      *>   slot (n, name)        the 10 most populated level-1 categories, most funds first (LOAD-DIMS)
      *>   fin  (k, txt, fx)     texts to fold: 'N' name text, 'C' category text, 'S1'..'S10' the slot names;
      *>                         fx = the text folded like NORM-TERM does (accents off, apostrophe -> _, upper case)
      *>   flt  (...)            one row: which conditions are on and their values (NAME-CLAUSE, CAT-CLAUSE and
      *>                         BUILD-WHERE decide the same things)
      *>   fv                    the view "fundos that pass the filter". It plays the part of
      *>                         "FROM fundos WHERE 1=1 <WS-WHERE>" of the desktop: the query programs read
      *>                         "FROM fv" and add their own conditions. Its text is FIXED; the values are
      *>                         bound (host variables) into pick and flt, never written into SQL.
      *> Needs FLT-REQ (req-filter) in the LINKAGE SECTION, and ws-open, p-db and SQLCA like p-db does.

      *> What every query program does first: the database (OPN-DB), then the filter (OPN-FILTER).
       OPN-OPEN.
           PERFORM OPN-DB
           IF OPN-STATUS = "OK"
               PERFORM OPN-FILTER
           END-IF.

       OPN-FILTER.
           PERFORM OPN-MAKE-PICKS
           IF OPN-STATUS = "OK"
               PERFORM OPN-MAKE-SLOTS
           END-IF
           IF OPN-STATUS = "OK"
               PERFORM OPN-MAKE-FOLDS
           END-IF
           IF OPN-STATUS = "OK"
               PERFORM OPN-MAKE-CONDITIONS
           END-IF
           IF OPN-STATUS = "OK"
               PERFORM OPN-MAKE-VIEW
           END-IF.

      *> The ticked names, one row each (MAKE-IN-LIST skips blank ones; so does this for the levels).
       OPN-MAKE-PICKS.
           EXEC SQL CREATE TEMP TABLE pick (fld TEXT, name TEXT, n INTEGER, PRIMARY KEY (fld, name)) END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM OPN-SQL-FAILED
               EXIT PARAGRAPH
           END-IF
           MOVE    0                             TO    SH-N2-ON
           MOVE    0                             TO    SH-N3-ON
           PERFORM VARYING SH-I FROM 1 BY 1 UNTIL SH-I > FLT-NAME-N
               MOVE    FLT-NAME-PICK (SH-I)      TO    SH-ONE
               EXEC SQL INSERT OR IGNORE INTO pick VALUES ('N', :SH-ONE, :SH-I) END-EXEC
           END-PERFORM
           PERFORM VARYING SH-I FROM 1 BY 1 UNTIL SH-I > FLT-CAT-N
               MOVE    FLT-CAT-PICK (SH-I)       TO    SH-ONE
               EXEC SQL INSERT OR IGNORE INTO pick VALUES ('C', :SH-ONE, :SH-I) END-EXEC
           END-PERFORM
           PERFORM VARYING SH-I FROM 1 BY 1 UNTIL SH-I > FLT-N2-N
               MOVE    FLT-N2-PICK (SH-I)        TO    SH-ONE
               IF SH-ONE NOT = SPACES
                   MOVE    1                     TO    SH-N2-ON
                   EXEC SQL INSERT OR IGNORE INTO pick VALUES ('2', :SH-ONE, :SH-I) END-EXEC
               END-IF
           END-PERFORM
           PERFORM VARYING SH-I FROM 1 BY 1 UNTIL SH-I > FLT-N3-N
               MOVE    FLT-N3-PICK (SH-I)        TO    SH-ONE
               IF SH-ONE NOT = SPACES
                   MOVE    1                     TO    SH-N3-ON
                   EXEC SQL INSERT OR IGNORE INTO pick VALUES ('3', :SH-ONE, :SH-I) END-EXEC
               END-IF
           END-PERFORM
           IF SQLSTATE(1:2) > "02"
               PERFORM OPN-SQL-FAILED
           END-IF.

      *> LOAD-DIMS: the 10 most populated level-1 categories, most funds first. A category keeps its slot (and
      *> its colour) under any filter, so every query matches its counts to these slots by name.
       OPN-MAKE-SLOTS.
           EXEC SQL CREATE TEMP TABLE slot (n INTEGER PRIMARY KEY, name TEXT) END-EXEC
           EXEC SQL
               DECLARE C-SLOT CURSOR FOR
                   SELECT nivel1 FROM fundos WHERE nivel1 IS NOT NULL
                    GROUP BY nivel1 ORDER BY COUNT(*) DESC
           END-EXEC
           EXEC SQL OPEN C-SLOT END-EXEC
           MOVE    0                             TO    SH-SLOT-N
           PERFORM UNTIL SQLSTATE NOT = "00000"
               EXEC SQL FETCH C-SLOT INTO :SH-SLOT-NAME END-EXEC
               IF SQLSTATE = "00000" AND SH-SLOT-N < 10
                   ADD     1                     TO    SH-SLOT-N
                   EXEC SQL INSERT INTO slot VALUES (:SH-SLOT-N, :SH-SLOT-NAME) END-EXEC
               END-IF
           END-PERFORM
           EXEC SQL CLOSE C-SLOT END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM OPN-SQL-FAILED
           END-IF.

      *> NORM-TERM, for the two typed texts and the 10 slot names: only the first 80 bytes of a text count (the
      *> desktop moves it into WS-TERM, PIC X(80)); accents off, the apostrophe becomes _, upper case ASCII-only
      *> (UPPER-CASE of a PIC X item); then TRIM, as WS-TERM::Trim() does. The table is NORM-TERM's, character
      *> for character: 24 lower case and 24 upper case accented letters and the apostrophe.
       OPN-MAKE-FOLDS.
           MOVE    FLT-NAME-TERM                 TO    SH-TERM-N
           MOVE    FLT-CAT-TERM                  TO    SH-TERM-C
           EXEC SQL CREATE TEMP TABLE fin (k TEXT PRIMARY KEY, txt TEXT, fx TEXT) END-EXEC
           EXEC SQL INSERT INTO fin VALUES ('N', :SH-TERM-N, NULL) END-EXEC
           EXEC SQL INSERT INTO fin VALUES ('C', :SH-TERM-C, NULL) END-EXEC
           EXEC SQL INSERT INTO fin SELECT 'S' || n, name, NULL FROM slot END-EXEC
           EXEC SQL UPDATE fin SET fx = txt END-EXEC
           *>   a / A <- á à â ã ä Á À Â Ã Ä
           EXEC SQL
               UPDATE fin SET fx = replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
                   fx, 'á', 'a'), 'à', 'a'), 'â', 'a'), 'ã', 'a'), 'ä', 'a'), 'Á', 'A'), 'À', 'A'), 'Â', 'A'), 'Ã', 'A'), 'Ä', 'A')
           END-EXEC
           *>   e / E <- é è ê ë É È Ê Ë
           EXEC SQL
               UPDATE fin SET fx = replace(replace(replace(replace(replace(replace(replace(replace(
                   fx, 'é', 'e'), 'è', 'e'), 'ê', 'e'), 'ë', 'e'), 'É', 'E'), 'È', 'E'), 'Ê', 'E'), 'Ë', 'E')
           END-EXEC
           *>   i / I <- í ì î ï Í Ì Î Ï
           EXEC SQL
               UPDATE fin SET fx = replace(replace(replace(replace(replace(replace(replace(replace(
                   fx, 'í', 'i'), 'ì', 'i'), 'î', 'i'), 'ï', 'i'), 'Í', 'I'), 'Ì', 'I'), 'Î', 'I'), 'Ï', 'I')
           END-EXEC
           *>   o / O <- ó ò ô õ ö Ó Ò Ô Õ Ö
           EXEC SQL
               UPDATE fin SET fx = replace(replace(replace(replace(replace(replace(replace(replace(replace(replace(
                   fx, 'ó', 'o'), 'ò', 'o'), 'ô', 'o'), 'õ', 'o'), 'ö', 'o'), 'Ó', 'O'), 'Ò', 'O'), 'Ô', 'O'), 'Õ', 'O'), 'Ö', 'O')
           END-EXEC
           *>   u / U <- ú ù û ü Ú Ù Û Ü
           EXEC SQL
               UPDATE fin SET fx = replace(replace(replace(replace(replace(replace(replace(replace(
                   fx, 'ú', 'u'), 'ù', 'u'), 'û', 'u'), 'ü', 'u'), 'Ú', 'U'), 'Ù', 'U'), 'Û', 'U'), 'Ü', 'U')
           END-EXEC
           *>   c / C <- ç Ç
           EXEC SQL
               UPDATE fin SET fx = replace(replace(
                   fx, 'ç', 'c'), 'Ç', 'C')
           END-EXEC
           *>   n / N <- ñ Ñ
           EXEC SQL
               UPDATE fin SET fx = replace(replace(
                   fx, 'ñ', 'n'), 'Ñ', 'N')
           END-EXEC
           *>   the apostrophe -> _ (the one-character LIKE wildcard), so it can never end a quoted pattern early
           EXEC SQL
               UPDATE fin SET fx = replace(
                   fx, '''', '_')
           END-EXEC
           *>   upper case (ASCII only, as UPPER-CASE of a PIC X item), then TRIM
           EXEC SQL UPDATE fin SET fx = trim(upper(fx)) END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM OPN-SQL-FAILED
               EXIT PARAGRAPH
           END-IF
           EXEC SQL SELECT fx INTO :OPN-NAME-KEY FROM fin WHERE k = 'N' END-EXEC
           EXEC SQL SELECT fx INTO :OPN-CAT-KEY FROM fin WHERE k = 'C' END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM OPN-SQL-FAILED
               EXIT PARAGRAPH
           END-IF

      *> SM-REFRESH for the categories: a slot is kept when the folded term is blank or the folded slot name
      *> contains it (instr > 0). The kept names stay in slot (popularity) order through the column n.
           EXEC SQL
               INSERT INTO pick
                   SELECT 'T', s.txt, CAST(SUBSTR(s.k, 2) AS INTEGER)
                     FROM fin s, fin c
                    WHERE c.k = 'C' AND s.k LIKE 'S%'
                      AND (c.fx = '' OR instr(s.fx, c.fx) > 0)
           END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM OPN-SQL-FAILED
           END-IF.

      *> BUILD-WHERE: which conditions are on, and their values. One row, bound from the request.
       OPN-MAKE-CONDITIONS.
      *>   NAME-CLAUSE: picked names win; else the typed text (when it is not blank once folded); else none.
           IF FLT-NAME-N > 0
               MOVE    1                         TO    SH-NAME-MODE
           ELSE
               IF OPN-NAME-KEY NOT = SPACES
                   MOVE    2                     TO    SH-NAME-MODE
               ELSE
                   MOVE    0                     TO    SH-NAME-MODE
               END-IF
           END-IF
      *>   CAT-CLAUSE: the same for the categories; the text path uses the slots 'T' it matches.
           IF FLT-CAT-N > 0
               MOVE    1                         TO    SH-CAT-MODE
           ELSE
               IF OPN-CAT-KEY NOT = SPACES
                   MOVE    2                     TO    SH-CAT-MODE
               ELSE
                   MOVE    0                     TO    SH-CAT-MODE
               END-IF
           END-IF
      *>   A date range left at its default (1970-01-01 to 2027-12-31) adds no test: a date test would drop
      *>   the funds that have no start date.
           MOVE    0                             TO    SH-DATE-ON
           MOVE    FLT-DATE-FROM                 TO    SH-DATE-LO
           MOVE    FLT-DATE-TO                   TO    SH-DATE-HI
           IF SH-DATE-LO NOT = SPACES AND SH-DATE-HI NOT = SPACES
               IF SH-DATE-LO NOT = "1970-01-01" OR SH-DATE-HI NOT = "2027-12-31"
                   MOVE    1                     TO    SH-DATE-ON
               END-IF
           END-IF
      *>   The PL range is in R$ million (hence * 1000000) and applies once it moved from 0 to 300000, so
      *>   the funds with no PL stay in until then.
           MOVE    0                             TO    SH-PL-ON
           COMPUTE SH-PL-LO = FLT-PL-FROM * 1000000
           COMPUTE SH-PL-HI = FLT-PL-TO * 1000000
           IF FLT-PL-FROM > 0 OR FLT-PL-TO < 300000
               MOVE    1                         TO    SH-PL-ON
           END-IF
           EXEC SQL
               CREATE TEMP TABLE flt (name_mode INTEGER, name_pat TEXT, cat_mode INTEGER,
                                      date_on INTEGER, date_lo TEXT, date_hi TEXT,
                                      pl_on INTEGER, pl_lo INTEGER, pl_hi INTEGER,
                                      n2_on INTEGER, n3_on INTEGER)
           END-EXEC
           EXEC SQL
               INSERT INTO flt
                   SELECT :SH-NAME-MODE, '%' || fx || '%', :SH-CAT-MODE,
                          :SH-DATE-ON, :SH-DATE-LO, :SH-DATE-HI,
                          :SH-PL-ON, :SH-PL-LO, :SH-PL-HI,
                          :SH-N2-ON, :SH-N3-ON
                     FROM fin WHERE k = 'N'
           END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM OPN-SQL-FAILED
           END-IF.

      *> "FROM fundos WHERE 1=1 ..." of every desktop query. A NULL never passes a test, as in the desktop
      *> (a fund with no start date is dropped by a date range, one with no PL by a PL range).
       OPN-MAKE-VIEW.
           EXEC SQL
               CREATE TEMP VIEW fv AS
                   SELECT f.* FROM flt CROSS JOIN fundos f
                    WHERE (flt.name_mode = 0
                           OR (flt.name_mode = 1 AND f.nome IN (SELECT name FROM pick WHERE fld = 'N'))
                           OR (flt.name_mode = 2 AND f.nome_key LIKE flt.name_pat))
                      AND (flt.cat_mode = 0
                           OR (flt.cat_mode = 1 AND f.nivel1 IN (SELECT name FROM pick WHERE fld = 'C'))
                           OR (flt.cat_mode = 2 AND f.nivel1 IN (SELECT name FROM pick WHERE fld = 'T')))
                      AND (flt.date_on = 0 OR (f.inicio >= flt.date_lo AND f.inicio <= flt.date_hi))
                      AND (flt.pl_on = 0 OR (f.pl >= flt.pl_lo AND f.pl <= flt.pl_hi))
                      AND (flt.n2_on = 0 OR f.nivel2 IN (SELECT name FROM pick WHERE fld = '2'))
                      AND (flt.n3_on = 0 OR f.nivel3 IN (SELECT name FROM pick WHERE fld = '3'))
           END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM OPN-SQL-FAILED
           END-IF.
