      *> The SELECTs of the files the sending and the approval use, with the alternate keys of the generated members.
           SELECT TOKEN-FILE ASSIGN TO "data/tokens.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS TOK-TOKEN
               ALTERNATE RECORD KEY IS TOK-QUO-ID WITH DUPLICATES
               FILE STATUS IS WS-FS-TOK.
           SELECT NOTIFY-FILE ASSIGN TO "data/notifications.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS NTF-ID
               ALTERNATE RECORD KEY IS NTF-QUO-ID WITH DUPLICATES
               FILE STATUS IS WS-FS-NTF.
           SELECT APPROVAL-FILE ASSIGN TO "data/approvals.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS APR-ID
               ALTERNATE RECORD KEY IS APR-QUO-ID WITH DUPLICATES
               FILE STATUS IS WS-FS-APR.
           SELECT CUSTOMER-FILE ASSIGN TO "data/customers.idx"
               ORGANIZATION IS INDEXED
               ACCESS MODE IS DYNAMIC
               RECORD KEY IS CUS-ID
               ALTERNATE RECORD KEY IS CUS-DOC WITH DUPLICATES
               FILE STATUS IS WS-FS-CUS.
