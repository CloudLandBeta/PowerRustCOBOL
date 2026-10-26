      *> FILE-CONTROL entry of the database script: the file the desktop reads in INIT-DB, one SQL statement per
      *> line. The path is relative to the folder the Backend was started in (the project folder, as the IDE does).
           SELECT SQL-FILE ASSIGN TO "data/fundos.sql"
               ORGANIZATION IS LINE SEQUENTIAL
               FILE STATUS IS SH-FS.
