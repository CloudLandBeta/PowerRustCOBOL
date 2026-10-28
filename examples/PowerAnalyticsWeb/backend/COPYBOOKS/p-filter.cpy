      *> The filter of the dashboard. The desktop keeps its pickers in tables of its database and pastes the rest of the
      *> filter into the text of its statements (BUILD-WHERE). Here the same tables are TEMPORARY tables of the call's
      *> connection (they vanish with it; nothing is written to fundos.db), declared exactly as INIT-DB declares them:
      *>   pick (fld, name)      the ticked names: 'N' company names, 'C' categories (PRIMARY KEY (fld, name))
      *>   vis  (fld, name)      'C': the categories the typed category text matches, in slot order (SM-REFRESH
      *>                         leaves them there on the desktop; here they are computed from the text)
      *> and the text of the filter is the desktop's text, with a ? where the desktop pastes a value (OPN-WHERE, values
      *> in SQLDA). Statements made from the same text over the same tables get the same plan from SQLite, and the
      *> plan decides what no SQL text says: the order of rows that tie on the ORDER BY, and the order in which
      *> SUM adds the floating-point PL values. Helper tables of the Backend alone:
      *>   slot (n, name)        the 10 most populated level-1 categories, most funds first (LOAD-DIMS)
      *>   fin  (k, txt, fx)     texts to fold: 'N' name text, 'C' category text, 'S1'..'S10' the slot names;
      *>                         fx = the text folded like NORM-TERM does (accents off, apostrophe -> _, upper case)
      *> No text of the request ever becomes SQL text: OPN-WHERE is made of fixed pieces, and every value is bound.
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
               PERFORM OPN-MAKE-WHERE
           END-IF.

      *> The ticked names, one row each, in the desktop's own tables (INIT-DB: pick and vis, same columns, same key).
      *> A ticked company name arrives as the byte of its fund id in FLT-NAME-FLAGS ("1"); the row of pick is that
      *> fund's name, the text the desktop stores for the ticked list row. One statement with the flags bound once:
      *> substr() of a BLOB reads byte k directly (on a TEXT it would walk the characters: 0.8 s instead of 3 ms).
      *> SH-PICK-N = the names in pick afterwards: NAME-CLAUSE of the desktop counts them the same way.
       OPN-MAKE-PICKS.
           EXEC SQL CREATE TEMP TABLE pick (fld TEXT, name TEXT, PRIMARY KEY (fld, name)) END-EXEC
           EXEC SQL CREATE TEMP TABLE vis (fld TEXT, name TEXT) END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM OPN-SQL-FAILED
               EXIT PARAGRAPH
           END-IF
           MOVE    0                             TO    SH-PICK-N
           IF FLT-NAME-N > 0
               EXEC SQL
                   INSERT OR IGNORE INTO pick
                       SELECT 'N', nome FROM fundos
                        WHERE substr(CAST(:FLT-NAME-FLAGS AS BLOB), id, 1) = x'31'
                          AND nome IS NOT NULL AND nome <> ''
               END-EXEC
               IF SQLSTATE(1:2) > "02"
                   PERFORM OPN-SQL-FAILED
                   EXIT PARAGRAPH
               END-IF
               EXEC SQL SELECT COUNT(*) INTO :SH-PICK-N FROM pick WHERE fld = 'N' END-EXEC
           END-IF
           PERFORM VARYING SH-I FROM 1 BY 1 UNTIL SH-I > FLT-CAT-N
               MOVE    FLT-CAT-PICK (SH-I)       TO    SH-ONE
               EXEC SQL INSERT OR IGNORE INTO pick VALUES ('C', :SH-ONE) END-EXEC
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
      *> contains it (instr > 0). The kept names go into vis in slot (popularity) order, as SM-REFRESH puts them.
           EXEC SQL
               INSERT INTO vis
                   SELECT 'C', s.txt
                     FROM fin s, fin c
                    WHERE c.k = 'C' AND s.k LIKE 'S%'
                      AND (c.fx = '' OR instr(s.fx, c.fx) > 0)
                    ORDER BY CAST(SUBSTR(s.k, 2) AS INTEGER)
           END-EXEC
           IF SQLSTATE(1:2) > "02"
               PERFORM OPN-SQL-FAILED
           END-IF.


      *> BUILD-WHERE of the desktop, piece for piece and in its order (NAME-CLAUSE, CAT-CLAUSE, the dates, the PL range,
      *> MAKE-IN-LIST for level 2 and level 3), into OPN-WHERE. Where the desktop pastes a value this writes ? and
      *> binds the value (OPN-ADD-PARAM); OPN-WN = how many. The desktop's list "IN ('a','b')" becomes "IN (?,?)":
      *> one ? per name, as many as the desktop's list has, so SQLite estimates both the same way.
       OPN-MAKE-WHERE.
           MOVE    SPACES                        TO    OPN-WHERE
           MOVE    1                             TO    OPN-WP
           MOVE    0                             TO    SQLDA-COUNT
           STRING  "WHERE 1=1" DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
      *>   NAME-CLAUSE: picked names win (in Todos mode the form sends none); else the folded text, when not blank.
           IF SH-PICK-N > 0
               STRING  " AND nome IN (SELECT name FROM pick WHERE fld = 'N')"
                   DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
           ELSE
               IF OPN-NAME-KEY NOT = SPACES
                   STRING  " AND nome_key LIKE ?" DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
                   MOVE    SPACES                TO    OPN-PV
                   STRING  "%" OPN-NAME-KEY::Trim() "%" DELIMITED BY SIZE INTO OPN-PV
                   PERFORM OPN-ADD-PARAM
               END-IF
           END-IF
      *>   CAT-CLAUSE: picked categories win; else the categories the text matched (vis), when the text is not blank.
           IF FLT-CAT-N > 0
               STRING  " AND nivel1 IN (SELECT name FROM pick WHERE fld = 'C')"
                   DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
           ELSE
               IF OPN-CAT-KEY NOT = SPACES
                   STRING  " AND nivel1 IN (SELECT name FROM vis WHERE fld = 'C')"
                       DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
               END-IF
           END-IF
      *>   A date range left at its default (1970-01-01 to 2027-12-31) adds no test: a date test would drop the
      *>   funds that have no start date.
           IF FLT-DATE-FROM NOT = SPACES AND FLT-DATE-TO NOT = SPACES
               IF FLT-DATE-FROM NOT = "1970-01-01" OR FLT-DATE-TO NOT = "2027-12-31"
                   STRING  " AND inicio >= ? AND inicio <= ?" DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
                   MOVE    FLT-DATE-FROM         TO    OPN-PV
                   PERFORM OPN-ADD-PARAM
                   MOVE    FLT-DATE-TO           TO    OPN-PV
                   PERFORM OPN-ADD-PARAM
               END-IF
           END-IF
      *>   The PL range is in R$ million (hence * 1000000) and applies once it moved from 0 to 300000, so the
      *>   funds with no PL stay in until then. The bounds are the digits the desktop writes (Z(14)9, trimmed).
           IF FLT-PL-FROM > 0 OR FLT-PL-TO < 300000
               STRING  " AND pl >= ? AND pl <= ?" DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
               COMPUTE OPN-PLX = FLT-PL-FROM * 1000000
               MOVE    OPN-PLX::Trim()           TO    OPN-PV
               PERFORM OPN-ADD-PARAM
               COMPUTE OPN-PLX = FLT-PL-TO * 1000000
               MOVE    OPN-PLX::Trim()           TO    OPN-PV
               PERFORM OPN-ADD-PARAM
           END-IF
      *>   MAKE-IN-LIST: the non-blank ticked names, trimmed, in the order they came; no list when none.
           MOVE    0                             TO    SH-N2-ON
           PERFORM VARYING SH-I FROM 1 BY 1 UNTIL SH-I > FLT-N2-N
               IF FLT-N2-PICK (SH-I) NOT = SPACES
                   IF SH-N2-ON = 0
                       STRING  " AND nivel2 IN (?" DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
                       MOVE    1                 TO    SH-N2-ON
                   ELSE
                       STRING  ",?" DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
                   END-IF
                   MOVE    FLT-N2-PICK (SH-I)::Trim() TO OPN-PV
                   PERFORM OPN-ADD-PARAM
               END-IF
           END-PERFORM
           IF SH-N2-ON = 1
               STRING  ")" DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
           END-IF
           MOVE    0                             TO    SH-N3-ON
           PERFORM VARYING SH-I FROM 1 BY 1 UNTIL SH-I > FLT-N3-N
               IF FLT-N3-PICK (SH-I) NOT = SPACES
                   IF SH-N3-ON = 0
                       STRING  " AND nivel3 IN (?" DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
                       MOVE    1                 TO    SH-N3-ON
                   ELSE
                       STRING  ",?" DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
                   END-IF
                   MOVE    FLT-N3-PICK (SH-I)::Trim() TO OPN-PV
                   PERFORM OPN-ADD-PARAM
               END-IF
           END-PERFORM
           IF SH-N3-ON = 1
               STRING  ")" DELIMITED BY SIZE INTO OPN-WHERE WITH POINTER OPN-WP
           END-IF
           MOVE    SQLDA-COUNT                   TO    OPN-WN.

      *> One more value for the next ?: OPN-PV becomes entry SQLDA-COUNT + 1, bound as text (the entry holds it, no
      *> pointer). SQLite gives a bound text the column's numeric affinity in pl >= ? and ano >= ?, as it does the
      *> desktop's pasted number.
       OPN-ADD-PARAM.
           ADD     1                             TO    SQLDA-COUNT
           MOVE    OPN-PV                        TO    SQLDA-VALUE (SQLDA-COUNT)
           MOVE    0                             TO    SQLDA-IND (SQLDA-COUNT)
           SET     SQLDA-DATA (SQLDA-COUNT)      TO    NULL
           SET     SQLDA-IND-PTR (SQLDA-COUNT)   TO    NULL.

      *> Before a statement: only the values of OPN-WHERE are bound; the statement's own come after (OPN-ADD-PARAM).
       OPN-WHERE-PARAMS.
           MOVE    OPN-WN                        TO    SQLDA-COUNT.
