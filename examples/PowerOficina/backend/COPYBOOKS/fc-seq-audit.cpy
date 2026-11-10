      *> FILE-CONTROL of the number sequences and the audit trail, with the file status each one reports.
           SELECT SEQUENCE-FILE ASSIGN TO "data/sequences.idx"
               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
               RECORD KEY IS SEQ-NAME FILE STATUS IS WS-FS-SEQ.
           SELECT AUDIT-FILE ASSIGN TO "data/audit.idx"
               ORGANIZATION IS INDEXED ACCESS MODE IS DYNAMIC
               RECORD KEY IS AUD-ID FILE STATUS IS WS-FS-AUD.
